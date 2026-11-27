// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.dpieczynski.sursum;

import android.webkit.CookieManager;
import android.webkit.ValueCallback;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.security.GeneralSecurityException;
import java.security.MessageDigest;
import java.security.SecureRandom;
import javax.crypto.Mac;
import javax.crypto.spec.SecretKeySpec;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Keeps the tablet's pairing when the device gets a new IP address (ADR 0028). The pairing
 * cookie belongs to the address the remote was loaded from; when the device turns up at another
 * address, the cookie is moved there, but only after the server at the new address proved that
 * it holds this pairing. A host that merely announces {@code _sursum._tcp} and copies the
 * device's public identity therefore never receives the cookie.
 *
 * <p>The proof: the app sends a fresh nonce and the pairing's public key (a hash of the token
 * hash); the server answers HMAC-SHA256, keyed with the token hash it stores, over its identity,
 * the address the request reached and the nonce. The token hash never leaves the device, and the
 * answer is accepted only for the address the app connected to, so relaying the question to the
 * real device does not help either. The values are pinned in
 * {@code crates/sursum-server/src/identity.rs} and {@code docs/protocol.md}.
 *
 * <p>The network methods block: call them off the main thread. The cookie methods belong on the
 * main thread.
 */
final class PairingMove {
    private static final String COOKIE = "sursum_device";
    private static final int TIMEOUT_MS = 3_000;
    /** Far more than the server's answers; anything bigger is not the device. */
    private static final int MAX_ANSWER = 4096;
    private static final SecureRandom RANDOM = new SecureRandom();

    private PairingMove() {}

    /** The identity of the server at this address, or null if it does not say. Blocks. */
    static String serverId(String address) {
        JSONObject identity = request(address, "/api/identity", null);
        String id = identity == null ? null : identity.optString("server_id", null);
        return id == null || id.isEmpty() ? null : id;
    }

    /**
     * Whether the server at {@code address} is {@code serverId} and holds the pairing whose
     * cookie value is {@code token}, proven for exactly this address. Blocks.
     */
    static boolean proves(String address, String serverId, String token) {
        String reachedHere = DeviceAddress.ipv4Endpoint(address);
        if (reachedHere == null || !serverId.equals(serverId(address))) {
            return false; // not an IPv4 address, or another device: the key is not even sent
        }
        try {
            String tokenHash = sha256Hex(token);
            String nonce = randomHex(16);
            JSONObject question = new JSONObject()
                    .put("key", sha256Hex("sursum-pairing-key\n" + tokenHash))
                    .put("nonce", nonce);
            JSONObject answer = request(address, "/api/identity/proof", question);
            if (answer == null || !serverId.equals(answer.optString("server_id"))) {
                return false;
            }
            // The server writes the address one way (as ipv4Endpoint does), so the string the
            // proof covers must be exactly the address this question went to.
            String reached = answer.optString("address");
            if (!reachedHere.equals(reached)) {
                return false; // an answer for another address, e.g. relayed from the real device
            }
            byte[] expected = hmacHex(tokenHash,
                    "sursum-pairing-proof\n" + serverId + "\n" + reached + "\n" + nonce);
            byte[] given = answer.optString("proof").getBytes(StandardCharsets.US_ASCII);
            return MessageDigest.isEqual(expected, given);
        } catch (JSONException | GeneralSecurityException impossible) {
            return false;
        }
    }

    /** The pairing token the WebView keeps for this address, or null if it is not paired there. */
    static String token(String address) {
        String cookies = CookieManager.getInstance().getCookie(origin(address));
        if (cookies == null) {
            return null;
        }
        for (String pair : cookies.split(";")) {
            String trimmed = pair.trim();
            if (trimmed.startsWith(COOKIE + "=") && trimmed.length() > COOKIE.length() + 1) {
                return trimmed.substring(COOKIE.length() + 1);
            }
        }
        return null;
    }

    /**
     * Copies the pairing to the new address, as the server would have set it there, then tells
     * {@code done} whether it worked. Only once it is in place does the old address lose it, and
     * only if that is an IPv4 address of another host ({@link #forgetsOld}).
     */
    static void move(String token, String from, String to, ValueCallback<Boolean> done) {
        CookieManager cookies = CookieManager.getInstance();
        cookies.setCookie(origin(to),
                COOKIE + "=" + token + "; Path=/; HttpOnly; SameSite=Strict; Max-Age=315360000",
                set -> {
                    if (!Boolean.TRUE.equals(set)) {
                        done.onReceiveValue(false); // the old address keeps it
                    } else if (forgetsOld(from, to)) {
                        cookies.setCookie(origin(from), COOKIE + "=; Path=/; Max-Age=0",
                                removed -> {
                                    cookies.flush();
                                    done.onReceiveValue(true);
                                });
                    } else {
                        cookies.flush();
                        done.onReceiveValue(true);
                    }
                });
    }

    /**
     * Whether the old address should lose the cookie after a move. An IPv4 address yes: DHCP may
     * give it to another machine, which must not receive the token. A name no: it stays the
     * device's, and keeping the cookie there keeps the name-first path for when it resolves
     * again. The same host never: cookies ignore the port, so deleting it there would delete
     * the copy just made.
     */
    private static boolean forgetsOld(String from, String to) {
        String old = DeviceAddress.ipv4Host(from);
        return old != null && !old.equals(DeviceAddress.ipv4Host(to));
    }

    private static String origin(String address) {
        return "http://" + address + "/";
    }

    /**
     * A small JSON request to the device: GET without a body, POST with one. Sends no cookie
     * (HttpURLConnection does not share the WebView's). Null unless the answer is 200 and JSON.
     */
    private static JSONObject request(String address, String path, JSONObject body) {
        HttpURLConnection connection = null;
        try {
            connection = (HttpURLConnection) new URL("http://" + address + path).openConnection();
            connection.setConnectTimeout(TIMEOUT_MS);
            connection.setReadTimeout(TIMEOUT_MS);
            connection.setInstanceFollowRedirects(false);
            connection.setUseCaches(false);
            if (body != null) {
                byte[] bytes = body.toString().getBytes(StandardCharsets.UTF_8);
                connection.setRequestMethod("POST");
                connection.setRequestProperty("Content-Type", "application/json");
                connection.setFixedLengthStreamingMode(bytes.length);
                connection.setDoOutput(true);
                try (OutputStream out = connection.getOutputStream()) {
                    out.write(bytes);
                }
            }
            if (connection.getResponseCode() != HttpURLConnection.HTTP_OK) {
                return null;
            }
            try (InputStream in = connection.getInputStream()) {
                return new JSONObject(readSmall(in));
            }
        } catch (IOException | JSONException | ClassCastException unreachable) {
            return null;
        } finally {
            if (connection != null) {
                connection.disconnect();
            }
        }
    }

    private static String readSmall(InputStream in) throws IOException {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        byte[] buffer = new byte[1024];
        int read;
        while ((read = in.read(buffer)) != -1) {
            out.write(buffer, 0, read);
            if (out.size() > MAX_ANSWER) {
                throw new IOException("answer too large");
            }
        }
        return out.toString("UTF-8");
    }

    private static String sha256Hex(String text) throws GeneralSecurityException {
        return hex(MessageDigest.getInstance("SHA-256")
                .digest(text.getBytes(StandardCharsets.UTF_8)));
    }

    /** HMAC-SHA256 keyed with the token hash's ASCII hex, as lowercase hex bytes. */
    private static byte[] hmacHex(String tokenHash, String message)
            throws GeneralSecurityException {
        Mac mac = Mac.getInstance("HmacSHA256");
        mac.init(new SecretKeySpec(tokenHash.getBytes(StandardCharsets.US_ASCII), "HmacSHA256"));
        return hex(mac.doFinal(message.getBytes(StandardCharsets.UTF_8)))
                .getBytes(StandardCharsets.US_ASCII);
    }

    private static String randomHex(int bytes) {
        byte[] nonce = new byte[bytes];
        RANDOM.nextBytes(nonce);
        return hex(nonce);
    }

    private static String hex(byte[] bytes) {
        StringBuilder text = new StringBuilder(bytes.length * 2);
        for (byte b : bytes) {
            text.append(Character.forDigit((b >> 4) & 0xf, 16))
                    .append(Character.forDigit(b & 0xf, 16));
        }
        return text.toString();
    }
}
