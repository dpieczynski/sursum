// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.dpieczynski.sursum;

import android.annotation.SuppressLint;
import android.app.Activity;
import android.app.DownloadManager;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.content.pm.ApplicationInfo;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.Environment;
import android.os.Handler;
import android.os.Looper;
import android.view.KeyEvent;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.view.WindowManager;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputMethodManager;
import android.webkit.CookieManager;
import android.webkit.URLUtil;
import android.webkit.ValueCallback;
import android.webkit.WebChromeClient;
import android.webkit.WebResourceError;
import android.webkit.WebResourceRequest;
import android.webkit.WebResourceResponse;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.TextView;
import android.widget.Toast;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/**
 * The app's only screen: the remote from the sursum device in a full-screen WebView that keeps
 * the tablet awake, and over it, until the remote has loaded, a small "find the device" view.
 * The app is a launcher and nothing more (hard rule 1): it knows no songs, sets or commands;
 * everything the operator works with comes from the server.
 */
public final class MainActivity extends Activity implements DeviceFinder.Listener {
    private static final int PICK_FILE = 1;
    /** How long to wait before trying an address again that did not answer. */
    private static final long RETRY_MS = 5_000;

    private final Handler main = new Handler(Looper.getMainLooper());
    private final Runnable retry = this::retryAddress;
    /** The few small requests that check a device's identity; never on the main thread. */
    private final ExecutorService network = Executors.newSingleThreadExecutor();
    private DeviceMemory memory;
    private FrameLayout root;
    private View findView;
    private TextView status;
    private EditText addressField;
    private DeviceFinder finder;
    private WebView web;
    /** The device being loaded or shown, as "host[:port]"; null before the first try. */
    private String address;
    /** Where to try next if loading from {@link #address} fails: the IP behind a name. */
    private String fallback;
    private boolean loading;
    private boolean loadFailed;
    /** True while checking whether a device at a new address is the one this tablet is paired with. */
    private boolean checking;
    /** Counts connection attempts, so a check that was overtaken does not connect any more. */
    private int attempt;
    private boolean resumed;
    private ValueCallback<Uri[]> pendingFiles;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        if (Build.VERSION.SDK_INT >= 28) {
            getWindow().getAttributes().layoutInDisplayCutoutMode =
                    WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
        }
        setContentView(R.layout.main);
        root = findViewById(android.R.id.content);
        findView = findViewById(R.id.find);
        status = findViewById(R.id.status);
        addressField = findViewById(R.id.address);
        root.setOnApplyWindowInsetsListener(this::keepAboveKeyboard);
        findViewById(R.id.connect).setOnClickListener(view -> connectTyped());
        addressField.setOnEditorActionListener((view, action, event) -> {
            connectTyped();
            return action == EditorInfo.IME_ACTION_GO;
        });
        if ((getApplicationInfo().flags & ApplicationInfo.FLAG_DEBUGGABLE) != 0) {
            WebView.setWebContentsDebuggingEnabled(true); // chrome://inspect over USB
        }
        memory = new DeviceMemory(getPreferences(MODE_PRIVATE));
        finder = new DeviceFinder(this, this);
        web = newWebView();
        if (Build.VERSION.SDK_INT >= 33) {
            getOnBackInvokedDispatcher().registerOnBackInvokedCallback(
                    android.window.OnBackInvokedDispatcher.PRIORITY_DEFAULT, this::goBack);
        }
        String remembered = memory.loaded();
        if (remembered != null) {
            addressField.setText(remembered);
            connect(remembered, null);
        } else {
            status.setText(R.string.searching);
        }
    }

    /**
     * The WebView for the remote; a new one replaces it if its renderer dies. JavaScript is the
     * remote itself (Leptos); the WebView opens no page but the device's.
     */
    @SuppressLint("SetJavaScriptEnabled")
    private WebView newWebView() {
        WebView view = new WebView(this);
        view.setBackgroundColor(getColor(R.color.background));
        WebSettings settings = view.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true); // the remote's per-tablet settings (pedal keys)
        // The remote sizes its own text; Android's font size setting must not scale it again.
        settings.setTextZoom(100);
        settings.setUserAgentString(settings.getUserAgentString() + " sursum-android/1");
        view.setWebViewClient(new RemoteClient());
        view.setWebChromeClient(new FilePicker());
        view.setDownloadListener((url, userAgent, disposition, mimeType, length) ->
                download(url, userAgent, URLUtil.guessFileName(url, disposition, mimeType)));
        root.addView(view, 0, new FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        return view;
    }

    private void connectTyped() {
        String typed = DeviceAddress.fromTyped(addressField.getText().toString());
        if (typed == null) {
            status.setText(R.string.bad_address);
            return;
        }
        addressField.clearFocus();
        getSystemService(InputMethodManager.class)
                .hideSoftInputFromWindow(addressField.getWindowToken(), 0);
        reach(null, typed);
    }

    /**
     * Connects to a device found on the network, typed in or retried. If this tablet is paired
     * with a device at another address, and the device here proves it is that device and holds
     * this pairing, the pairing cookie moves here first, so the remote opens without the PIN
     * after DHCP gave the device a new address (ADR 0028). Otherwise it connects as before: by
     * name first when there is one. A name this tablet is paired under is always tried first;
     * only if it does not load does the fallback to the IP address come back here and move.
     */
    private void reach(String byName, String byAddress) {
        memory.notePairing();
        String from = memory.paired();
        String server = memory.pairedServer();
        String token = from == null || server == null ? null : PairingMove.token(from);
        boolean pairedByName = byName != null && PairingMove.token(byName) != null;
        if (token == null || pairedByName || DeviceAddress.ipv4Endpoint(byAddress) == null
                || token.equals(PairingMove.token(byAddress))) {
            connectFound(byName, byAddress);
            return;
        }
        main.removeCallbacks(retry);
        loading = false;
        web.stopLoading(); // an earlier load must not finish over the check
        checking = true;
        int mine = ++attempt;
        addressField.setText(byAddress);
        status.setText(getString(R.string.connecting, byAddress));
        network.execute(() -> {
            boolean same = PairingMove.proves(byAddress, server, token);
            main.post(() -> {
                if (mine != attempt || isDestroyed()) {
                    return; // overtaken by another connection, or the app is closing
                }
                if (!same) {
                    connectFound(byName, byAddress);
                    return;
                }
                // Still checking until the cookie is in place: nothing else may connect meanwhile.
                PairingMove.move(token, from, byAddress, moved -> {
                    if (moved) {
                        memory.setPaired(byAddress, server);
                    }
                    if (mine == attempt && !isDestroyed()) {
                        connect(byAddress, null);
                    }
                });
            });
        });
    }

    private void connectFound(String byName, String byAddress) {
        if (byName != null) {
            connect(byName, byAddress);
        } else {
            connect(byAddress, null);
        }
    }

    /**
     * Loads the remote from this address behind the find view, which stays until it loads; if
     * it fails and there is a fallback, that is tried at once.
     */
    private void connect(String target, String then) {
        main.removeCallbacks(retry);
        checking = false;
        attempt++;
        address = target;
        fallback = then;
        addressField.setText(target);
        loading = true;
        loadFailed = false;
        status.setText(getString(R.string.connecting, target));
        web.loadUrl(DeviceAddress.remoteUrl(target));
    }

    /**
     * A found device is loaded by name first, so its pairing outlives a new IP address where
     * the name resolves; where it does not, the pairing moves to the new address ({@link #reach}).
     */
    @Override
    public void onFound(String byName, String byAddress) {
        boolean busy = checking || (loading && (byAddress.equals(address)
                || byAddress.equals(fallback) || (byName != null && byName.equals(address))));
        if (findView.getVisibility() == View.VISIBLE && !addressField.hasFocus() && !busy) {
            reach(byName, byAddress);
        }
    }

    private void retryAddress() {
        if (findView.getVisibility() == View.VISIBLE && !addressField.hasFocus() && !loading
                && !checking && address != null) {
            reach(null, address); // a typed new address may come up later: still move then
        }
    }

    private void showRemote() {
        findView.setVisibility(View.GONE);
        finder.stop();
        memory.setLoaded(address);
        learnServer(address);
        web.clearHistory(); // back never leaves the remote for an earlier device or error page
        web.requestFocus(); // pedal key presses go to the page
    }

    /** Remembers which server answers at the address the remote was loaded from. */
    private void learnServer(String loaded) {
        network.execute(() -> {
            String server = PairingMove.serverId(loaded);
            main.post(() -> {
                if (server != null) {
                    memory.setLoadedServer(loaded, server);
                }
            });
        });
    }

    private void showFind(String message) {
        status.setText(message);
        findView.setVisibility(View.VISIBLE);
        if (resumed) {
            finder.start();
            main.postDelayed(retry, RETRY_MS);
        }
    }

    private final class RemoteClient extends WebViewClient {
        @Override
        public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest request) {
            String url = request.getUrl().toString();
            if (DeviceAddress.isOnDevice(url, address)) {
                return false;
            }
            try {
                startActivity(new Intent(Intent.ACTION_VIEW, request.getUrl()));
            } catch (ActivityNotFoundException noBrowser) {
                // Nothing to open it with; stay on the remote.
            }
            return true;
        }

        @Override
        public void onReceivedError(WebView view, WebResourceRequest request,
                WebResourceError error) {
            if (isCurrentPage(request)) {
                loadFailed = true;
            }
        }

        @Override
        public void onReceivedHttpError(WebView view, WebResourceRequest request,
                WebResourceResponse response) {
            if (isCurrentPage(request)) {
                loadFailed = true; // something answers there, but not the remote
            }
        }

        /** The page of the device being loaded, not an earlier one that was cut short. */
        private boolean isCurrentPage(WebResourceRequest request) {
            return request.isForMainFrame()
                    && DeviceAddress.isOnDevice(request.getUrl().toString(), address);
        }

        @Override
        public void onPageFinished(WebView view, String url) {
            if (!loading || !DeviceAddress.isOnDevice(url, address)) {
                return;
            }
            loading = false;
            if (loadFailed && fallback != null) {
                reach(null, fallback); // the name did not load: the pairing may move to the IP
            } else if (loadFailed) {
                showFind(getString(R.string.cannot_connect, address));
            } else {
                showRemote();
            }
        }

        @Override
        public boolean onRenderProcessGone(WebView view,
                android.webkit.RenderProcessGoneDetail detail) {
            // Without this the whole app would be killed with the renderer.
            root.removeView(view);
            view.destroy();
            web = newWebView();
            findView.setVisibility(View.VISIBLE);
            if (address != null) {
                connect(address, null);
            }
            return true;
        }
    }

    /** Lets the remote's file fields (logo, OpenSong import, merge, backup, update) work. */
    private final class FilePicker extends WebChromeClient {
        @Override
        public boolean onShowFileChooser(WebView view, ValueCallback<Uri[]> callback,
                FileChooserParams params) {
            if (pendingFiles != null) {
                pendingFiles.onReceiveValue(null);
            }
            pendingFiles = callback;
            // Any type: the remote's accept lists name extensions such as ".sursum", which
            // Android's MIME-based pickers would turn into "nothing matches".
            Intent pick = new Intent(Intent.ACTION_GET_CONTENT)
                    .addCategory(Intent.CATEGORY_OPENABLE)
                    .setType("*/*");
            try {
                startActivityForResult(pick, PICK_FILE);
            } catch (ActivityNotFoundException noPicker) {
                pendingFiles = null;
                Toast.makeText(MainActivity.this, R.string.no_file_picker, Toast.LENGTH_LONG)
                        .show();
                return false;
            }
            return true;
        }
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        if (requestCode == PICK_FILE && pendingFiles != null) {
            pendingFiles.onReceiveValue(
                    WebChromeClient.FileChooserParams.parseResult(resultCode, data));
            pendingFiles = null;
        }
    }

    /** Saves a library export or a backup to Downloads, with the pairing cookie attached. */
    private void download(String url, String userAgent, String fileName) {
        DownloadManager.Request request = new DownloadManager.Request(Uri.parse(url))
                .addRequestHeader("Cookie", CookieManager.getInstance().getCookie(url))
                .addRequestHeader("User-Agent", userAgent)
                .setTitle(fileName)
                .setNotificationVisibility(
                        DownloadManager.Request.VISIBILITY_VISIBLE_NOTIFY_COMPLETED);
        if (Build.VERSION.SDK_INT >= 29) {
            request.setDestinationInExternalPublicDir(Environment.DIRECTORY_DOWNLOADS, fileName);
        } else {
            // The shared Downloads folder needs a storage permission before Android 10.
            request.setDestinationInExternalFilesDir(this, Environment.DIRECTORY_DOWNLOADS,
                    fileName);
        }
        getSystemService(DownloadManager.class).enqueue(request);
        Toast.makeText(this, getString(R.string.downloading, fileName), Toast.LENGTH_SHORT)
                .show();
    }

    /** Back moves between the remote's views (its URL fragments) and never leaves the app. */
    private void goBack() {
        if (findView.getVisibility() != View.VISIBLE && web.canGoBack()) {
            web.goBack();
        }
    }

    /** Android 12 and older; 13 and later use the callback registered in onCreate. */
    @Override
    @SuppressWarnings("deprecation")
    @SuppressLint("GestureBackNavigation")
    public void onBackPressed() {
        goBack();
    }

    @Override
    public boolean dispatchKeyEvent(KeyEvent event) {
        // Android turns an Escape the page did not handle into Back; a pedal or a keyboard must
        // not switch views that way.
        if (event.getKeyCode() == KeyEvent.KEYCODE_BACK
                && (event.getFlags() & KeyEvent.FLAG_FALLBACK) != 0) {
            return true;
        }
        return super.dispatchKeyEvent(event);
    }

    /** The on-screen keyboard shrinks the page instead of covering the field being edited. */
    @SuppressWarnings("deprecation")
    private WindowInsets keepAboveKeyboard(View view, WindowInsets insets) {
        int bottom = Build.VERSION.SDK_INT >= 30
                ? insets.getInsets(WindowInsets.Type.ime()).bottom
                : insets.getSystemWindowInsetBottom();
        view.setPadding(0, 0, 0, bottom);
        return insets;
    }

    @SuppressWarnings("deprecation")
    private void enterImmersive() {
        if (Build.VERSION.SDK_INT >= 30) {
            WindowInsetsController controller = getWindow().getInsetsController();
            controller.hide(WindowInsets.Type.systemBars());
            controller.setSystemBarsBehavior(
                    WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
        } else {
            getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                    | View.SYSTEM_UI_FLAG_FULLSCREEN | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                    | View.SYSTEM_UI_FLAG_LAYOUT_STABLE | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                    | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION);
        }
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) {
            enterImmersive();
        }
    }

    @Override
    protected void onResume() {
        super.onResume();
        resumed = true;
        web.onResume();
        enterImmersive();
        if (findView.getVisibility() == View.VISIBLE) {
            finder.start();
            main.postDelayed(retry, RETRY_MS);
        }
    }

    @Override
    protected void onPause() {
        resumed = false;
        main.removeCallbacks(retry);
        finder.stop();
        web.onPause();
        CookieManager.getInstance().flush(); // the pairing cookie survives a killed app
        memory.notePairing(); // the operator may have paired since the remote loaded
        super.onPause();
    }

    @Override
    protected void onDestroy() {
        network.shutdownNow();
        web.destroy();
        super.onDestroy();
    }
}
