// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.dpieczynski.sursum;

import android.content.SharedPreferences;

/**
 * What the app remembers between starts. Two things that usually coincide but must not be
 * mixed up (ADR 0028):
 *
 * <ul>
 *   <li>where the remote last loaded, with the identity of the server there: the app opens that
 *       address first at the next start;
 *   <li>where the pairing cookie lives, with the identity of the server it belongs to: the
 *       pairing is moved from there. Loading an unpaired page (another church's device, or a
 *       host pretending to be this one) changes the first but never the second, so it cannot
 *       strand the real pairing.
 * </ul>
 */
final class DeviceMemory {
    /** The key the app has used since its first build; kept so installed tablets keep it. */
    private static final String LOADED = "address";
    private static final String LOADED_SERVER = "address_server";
    private static final String PAIRED = "paired";
    private static final String PAIRED_SERVER = "paired_server";

    private final SharedPreferences prefs;

    DeviceMemory(SharedPreferences prefs) {
        this.prefs = prefs;
    }

    /** Where the remote last loaded, or null before the first time. */
    String loaded() {
        return prefs.getString(LOADED, null);
    }

    /** The remote loaded here; another address may be another server, so its identity is unknown. */
    void setLoaded(String address) {
        if (!address.equals(loaded())) {
            prefs.edit().putString(LOADED, address).remove(LOADED_SERVER).apply();
        }
    }

    /** The identity the server at a loaded address gave, if that is still the loaded address. */
    void setLoadedServer(String address, String server) {
        if (address.equals(loaded())) {
            prefs.edit().putString(LOADED_SERVER, server).apply();
            notePairing();
        }
    }

    /** Where the pairing cookie lives, or null if the app never saw this tablet paired. */
    String paired() {
        return prefs.getString(PAIRED, null);
    }

    /** The identity of the server the pairing belongs to. */
    String pairedServer() {
        return prefs.getString(PAIRED_SERVER, null);
    }

    /** The pairing now lives at this address (after the app moved it there). */
    void setPaired(String address, String server) {
        prefs.edit().putString(PAIRED, address).putString(PAIRED_SERVER, server).apply();
    }

    /**
     * Takes the loaded address as the pairing's home once the WebView holds a pairing cookie for
     * it and the server's identity there is known. The page sets the cookie itself when the
     * operator enters the PIN, after it loaded, so this is checked again on pause and before
     * every move.
     */
    void notePairing() {
        String address = loaded();
        String server = prefs.getString(LOADED_SERVER, null);
        if (address != null && server != null && PairingMove.token(address) != null) {
            setPaired(address, server);
        }
    }
}
