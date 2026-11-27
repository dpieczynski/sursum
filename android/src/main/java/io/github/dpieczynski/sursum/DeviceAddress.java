// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.dpieczynski.sursum;

import java.net.Inet6Address;
import java.net.InetAddress;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Where the sursum device answers, as "host" or "host:port" (port 80 left out). The app keeps it
 * in this form, remembers the last one that worked and turns it into the remote's URL.
 */
final class DeviceAddress {
    /** One DNS label: what Avahi's "%h" puts in the service name (the device's host name). */
    private static final Pattern LABEL =
            Pattern.compile("[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?");
    /** A host name, an IPv4 address or a bracketed IPv6 address, with an optional port. */
    private static final Pattern VALID =
            Pattern.compile("([A-Za-z0-9.-]+|\\[[0-9A-Fa-f:.]+\\])(:[0-9]{1,5})?");
    /** An IPv4 address with an optional port. */
    private static final Pattern IPV4 = Pattern.compile(
            "([0-9]{1,3})\\.([0-9]{1,3})\\.([0-9]{1,3})\\.([0-9]{1,3})(?::([0-9]{1,5}))?");

    private DeviceAddress() {}

    /** The remote on the device. Always plain HTTP (ADR 0007). */
    static String remoteUrl(String address) {
        return "http://" + address + "/remote/";
    }

    /**
     * Whether a URL belongs to the device at this address. Other links (the licence pages'
     * links to the internet) open in the system browser.
     */
    static boolean isOnDevice(String url, String address) {
        return address != null && url.startsWith("http://" + address + "/");
    }

    /**
     * What the operator typed, made into an address: "192.168.1.20", "sursum.local:8080" or a
     * pasted "http://192.168.1.20/remote/" all work. Null when it cannot be an address.
     */
    static String fromTyped(String typed) {
        String text = typed.trim();
        int scheme = text.indexOf("://");
        if (scheme >= 0) {
            text = text.substring(scheme + 3);
        }
        int path = text.indexOf('/');
        if (path >= 0) {
            text = text.substring(0, path);
        }
        if (text.endsWith(":80")) {
            text = text.substring(0, text.length() - 3);
        }
        return VALID.matcher(text).matches() ? text : null;
    }

    /**
     * The device's mDNS name, from the name it announces its service under ("sursum" becomes
     * "sursum.local"). Null when that name cannot be a host name, e.g. after Avahi renamed a
     * clashing service to "sursum #2". The name is preferred over the IP address because the
     * pairing cookie belongs to the host the remote was loaded from: under a name it survives
     * the device getting a new address from DHCP.
     */
    static String fromServiceName(String serviceName, int port) {
        if (serviceName == null || !LABEL.matcher(serviceName).matches()) {
            return null;
        }
        String name = serviceName + ".local";
        return port == 80 ? name : name + ":" + port;
    }

    /**
     * An IPv4 address written one way, always with its port: "192.168.1.20" and
     * "192.168.1.20:80" both become "192.168.1.20:80", as the server writes the address a
     * pairing proof was given for. Null for anything else (names, IPv6): the pairing only moves
     * to an IPv4 address, which is what DHCP changes and what discovery reports first.
     */
    static String ipv4Endpoint(String address) {
        if (address == null) {
            return null;
        }
        Matcher parts = IPV4.matcher(address);
        if (!parts.matches()) {
            return null;
        }
        StringBuilder text = new StringBuilder();
        for (int i = 1; i <= 4; i++) {
            int octet = Integer.parseInt(parts.group(i));
            if (octet > 255) {
                return null;
            }
            text.append(i > 1 ? "." : "").append(octet);
        }
        int port = parts.group(5) == null ? 80 : Integer.parseInt(parts.group(5));
        if (port < 1 || port > 65535) {
            return null;
        }
        return text.append(':').append(port).toString();
    }

    /** The IPv4 address without its port ("192.168.1.20"), or null when it is not one. */
    static String ipv4Host(String address) {
        String endpoint = ipv4Endpoint(address);
        return endpoint == null ? null : endpoint.substring(0, endpoint.indexOf(':'));
    }

    /** The address of a device found on the network. */
    static String fromResolved(InetAddress host, int port) {
        String name = host.getHostAddress();
        if (host instanceof Inet6Address) {
            name = "[" + name + "]";
        }
        return port == 80 ? name : name + ":" + port;
    }
}
