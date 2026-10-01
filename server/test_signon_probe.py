import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import signon_probe as probe


class SignonListenerTests(unittest.TestCase):
    def test_shared_upload_addresses_have_one_listener_and_canonical_fallback(self):
        original_hosts = dict(probe.hosts.HTTPS_HOSTS)
        bindings = probe.listener_bindings("127.0.0.1", [443, 80], destiny_hosts=True)
        endpoints = [(addr, port) for addr, port, _ in bindings]
        expected = {(addr, port) for addr in {"127.0.0.1", *original_hosts.values()} for port in (443, 80)}
        self.assertEqual(set(endpoints), expected)
        self.assertEqual(len(endpoints), len(expected))
        for addr, canonical in (("127.0.1.9", "www9.signon.deadorbit.net"),
                                ("127.0.2.9", "www9.signon.gravityshavings.net")):
            self.assertEqual([(port, host) for ip, port, host in bindings if ip == addr],
                             [(443, canonical), (80, canonical)])
        self.assertEqual(probe.hosts.HTTPS_HOSTS, original_hosts)

    def test_custom_bind_and_duplicate_ports_keep_first_binding(self):
        bindings = probe.listener_bindings("127.0.1.9", [443, 443], destiny_hosts=True)
        self.assertEqual([entry for entry in bindings if entry[0] == "127.0.1.9"],
                         [("127.0.1.9", 443, None)])
        self.assertEqual(probe.listener_bindings("127.0.0.1", [443]),
                         [("127.0.0.1", 443, None)])

    def test_main_starts_one_thread_per_endpoint(self):
        with (mock.patch.object(probe.sys, "argv", ["signon_probe.py", "--destiny-hosts"]),
              mock.patch.object(probe, "CertStore"),
              mock.patch.object(probe.threading, "Thread") as thread):
            probe.main()
        started = [call.kwargs["args"] for call in thread.call_args_list]
        endpoints = [(args[0], args[1]) for args in started]
        expected = {(addr, port) for addr in {"127.0.0.1", *probe.hosts.HTTPS_HOSTS.values()}
                    for port in (443, 80)}
        self.assertEqual(set(endpoints), expected)
        self.assertEqual(len(endpoints), len(expected))
        self.assertEqual(thread.return_value.start.call_count, len(expected))
        self.assertEqual(thread.return_value.join.call_count, len(expected))

    def test_alias_sni_and_http_host_survive_canonical_listener(self):
        alias, canonical = "dm-partnernet.upload.deadorbit.net", "www9.signon.deadorbit.net"
        request = (f"GET /ticket_drop HTTP/1.1\r\nHost: {alias}\r\n\r\n").encode()
        parsed = ("GET /ticket_drop HTTP/1.1", {"host": alias}, b"")
        for sni, certificate_host in ((alias, alias), (None, canonical)):
            with self.subTest(sni=sni), tempfile.TemporaryDirectory() as temp:
                args = SimpleNamespace(plain_port=80, mode="terminate", default_host=canonical,
                                       timeout=1, capture_dir=Path(temp), manifest_url="", status=503)
                raw, certs, tls = mock.Mock(), mock.Mock(), mock.Mock()
                tls.version.return_value = "TLSv1.2"
                tls.cipher.return_value = ("cipher", "TLSv1.2", 128)
                certs.context_for.return_value.wrap_socket.return_value = tls
                with (mock.patch.object(probe, "peek_record", return_value=b"hello"),
                      mock.patch.object(probe, "describe_hello", return_value={
                          "sni": sni, "client_version": "TLS1.2"}),
                      mock.patch.object(probe, "read_http_request", return_value=(request, parsed)),
                      mock.patch.object(probe, "log") as log):
                    probe.handle(raw, ("127.0.0.1", 12345), 443, args, certs, canonical)
                certs.context_for.assert_called_once_with(certificate_host)
                captures = list(Path(temp).glob("*.http"))
                self.assertEqual(len(captures), 1)
                self.assertEqual(captures[0].read_bytes(), request)
                self.assertTrue(any(f"host='{alias}'" in call.args[1] for call in log.call_args_list))
                tls.sendall.assert_called_once_with(
                    b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                raw.close.assert_called_once()


if __name__ == "__main__":
    unittest.main()
