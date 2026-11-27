// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.dpieczynski.sursum;

import android.content.Context;
import android.net.nsd.NsdManager;
import android.net.nsd.NsdServiceInfo;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import java.net.Inet4Address;
import java.net.InetAddress;
import java.util.ArrayDeque;
import java.util.Collections;
import java.util.List;

/**
 * Finds sursum devices on the local network through DNS-SD: the device image announces
 * {@code _sursum._tcp} with Avahi (image/layer/sursum-device.d/.../avahi/services/sursum.service).
 * Every found device is resolved and reported on the main thread, by its mDNS name when the
 * service name allows it and by its IP address.
 */
final class DeviceFinder {
    /** Reports a device found on the network, on the main thread. */
    interface Listener {
        /**
         * @param byName "sursum.local" (null if the service name is not a host name)
         * @param byAddress its IP address, for when the name does not resolve on this tablet
         */
        void onFound(String byName, String byAddress);
    }

    private static final String SERVICE_TYPE = "_sursum._tcp";

    private final NsdManager nsd;
    private final Listener listener;
    private final Handler main = new Handler(Looper.getMainLooper());
    /** Services waiting to be resolved: older Androids resolve one at a time. */
    private final ArrayDeque<NsdServiceInfo> unresolved = new ArrayDeque<>();
    /** The running discovery, or null; a listener object serves one discovery only. */
    private NsdManager.DiscoveryListener discovery;
    private boolean resolving;

    DeviceFinder(Context context, Listener listener) {
        this.nsd = (NsdManager) context.getSystemService(Context.NSD_SERVICE);
        this.listener = listener;
    }

    /** Starts looking, unless already looking. */
    void start() {
        if (discovery != null) {
            return;
        }
        NsdManager.DiscoveryListener started = new Discovery();
        discovery = started;
        nsd.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, started);
    }

    /** Stops looking; results still on their way are dropped. */
    void stop() {
        if (discovery == null) {
            return;
        }
        try {
            nsd.stopServiceDiscovery(discovery);
        } catch (IllegalArgumentException alreadyStopped) {
            // Discovery failed to start or the system already ended it.
        }
        discovery = null;
        unresolved.clear();
    }

    private final class Discovery implements NsdManager.DiscoveryListener {
        @Override
        public void onServiceFound(NsdServiceInfo service) {
            main.post(() -> {
                if (discovery == this) {
                    unresolved.add(service);
                    resolveNext();
                }
            });
        }

        @Override
        public void onStartDiscoveryFailed(String serviceType, int errorCode) {
            main.post(() -> {
                if (discovery == this) {
                    discovery = null;
                }
            });
        }

        @Override
        public void onDiscoveryStarted(String serviceType) {}

        @Override
        public void onDiscoveryStopped(String serviceType) {}

        @Override
        public void onServiceLost(NsdServiceInfo service) {}

        @Override
        public void onStopDiscoveryFailed(String serviceType, int errorCode) {}
    }

    // resolveService is deprecated from API 34 in favour of registerServiceInfoCallback, which
    // older tablets lack; it still works and resolves once, which is all we need.
    @SuppressWarnings("deprecation")
    private void resolveNext() {
        if (resolving || unresolved.isEmpty()) {
            return;
        }
        resolving = true;
        nsd.resolveService(unresolved.poll(), new NsdManager.ResolveListener() {
            @Override
            public void onServiceResolved(NsdServiceInfo service) {
                InetAddress host = usableAddress(service);
                main.post(() -> {
                    resolving = false;
                    if (host != null && discovery != null) {
                        listener.onFound(
                                DeviceAddress.fromServiceName(service.getServiceName(),
                                        service.getPort()),
                                DeviceAddress.fromResolved(host, service.getPort()));
                    }
                    resolveNext();
                });
            }

            @Override
            public void onResolveFailed(NsdServiceInfo service, int errorCode) {
                main.post(() -> {
                    resolving = false;
                    resolveNext();
                });
            }
        });
    }

    /**
     * IPv4 first: a link-local IPv6 address needs a zone in the URL, which WebView does not
     * take. Null when the device announced nothing usable.
     */
    @SuppressWarnings("deprecation")
    private static InetAddress usableAddress(NsdServiceInfo service) {
        List<InetAddress> hosts = Build.VERSION.SDK_INT >= 34
                ? service.getHostAddresses()
                : Collections.singletonList(service.getHost());
        InetAddress fallback = null;
        for (InetAddress host : hosts) {
            if (host instanceof Inet4Address) {
                return host;
            }
            if (host != null && !host.isLinkLocalAddress() && fallback == null) {
                fallback = host;
            }
        }
        return fallback;
    }
}
