# Importing a packet capture

EmuLabZip can open a `.pcap` or `.pcapng` file and show what it holds the way it shows live traffic. It is useful when
you cannot use the proxy: an app that ignores proxy settings, traffic from a system service, or a capture somebody sent
you.

```bash
emulabzip net import capture.pcap            # lists the requests
emulabzip net import capture.pcap --dns      # and the DNS lookups
emulabzip net import capture.pcap --har out.har
```

In the app, press **Import** in the Network view. The requests join the list with a `PCAP` badge, and a banner shows
what the file held, with the DNS lookups one click away.

## What comes out

| In the capture | In EmuLabZip |
|----------------|--------------|
| Plain HTTP/1.x on TCP | Requests and responses, with headers and bodies (chunked and compressed ones decoded) and timing |
| A TLS connection | A **connection** named by the server name in its first message (SNI), with the bytes each way |
| Any other TCP connection | A connection to its address |
| DNS over UDP port 53 | A list of lookups with their answers and status |

TLS stays encrypted, so its contents are not available from a capture; use the proxy for that. TCP streams are put back
together by sequence number: out of order and retransmitted segments are handled, and a missing piece is reported
instead of guessed ("some packets are missing"). Fragmented IP packets are skipped. Files up to 512 MB are read.

Ethernet, Linux "cooked" (`any` interface), raw IP and loopback captures are understood, in every byte order and with
nanosecond timestamps.

## Capturing

On an Android emulator, start it with `-tcpdump`:

```bash
emulator -avd Pixel_9 -tcpdump capture.pcap
```

Everything the emulated network carries is written to the file, and it is complete once the emulator exits. On a
rooted image or device you can use `tcpdump` from the shell: `adb shell tcpdump -i any -w /sdcard/capture.pcap`, then
`adb pull` it. For a Mac, `tcpdump -i lo0 -w capture.pcap` on the loopback interface captures a simulator's traffic.
