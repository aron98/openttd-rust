# SPDX-License-Identifier: GPL-2.0-only
"""Native protocol boundary tests. Run: python3 -m unittest discover -s scripts/tests."""

import struct
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import socket

from native_contract_content import grf_bytes
from native_contract_protocol import ContractError, parse_game_info, receive_packet


def info_packet() -> bytes:
    payload = (
        b"\x06\x07"
        + struct.pack("<Q", 123)
        + b"\x01"
        + b"\xff\xff\xff\xff\x00\x00"
        + struct.pack("<II", 712223, 712223)
        + b"\x0f\x01\x19"
        + b"Contract server\x0015.3\x00"
        + b"\x00\x19\x01\x01"
        + struct.pack("<HH", 64, 64)
        + b"\x00\x01"
    )
    return struct.pack("<H", len(payload) + 2) + payload


class GameInfoTests(unittest.TestCase):
    def test_decodes_schema7_vanilla_packet(self) -> None:
        # Given a server game-info message in the pinned wire format.
        packet = info_packet()
        # When decoded.
        info = parse_game_info(packet)
        # Then identity, world dimensions and content are explicit.
        self.assertEqual(info.revision, "15.3")
        self.assertEqual((info.width, info.height), (64, 64))
        self.assertEqual(info.grfs, ())
        self.assertEqual(info.ticks, 123)

    def test_decodes_actual_original_server_content_packet(self) -> None:
        packet = (
            Path(__file__).resolve().parents[2] / "fixtures/contracts/game-info-modded.bin"
        ).read_bytes()
        info = parse_game_info(packet)
        self.assertEqual(info.revision, "15.3")
        self.assertEqual(len(info.grfs), 1)
        self.assertEqual(info.grfs[0].grfid, "52555354")
        self.assertEqual(info.grfs[0].md5, "a5039c06887193868752bdec1694de4c")
        self.assertEqual(info.grfs[0].name, "Contract Speed")

    def test_rejects_unsupported_content_serialization(self) -> None:
        packet = bytearray(info_packet())
        packet[12] = 0
        with self.assertRaises(ContractError):
            parse_game_info(bytes(packet))

    def test_rejects_oversized_socket_packet_before_body(self) -> None:
        first, second = socket.socketpair()
        with first, second:
            first.sendall(b"\xff\xff")
            with self.assertRaises(ContractError):
                receive_packet(second)

    def test_generated_content_matches_committed_binary(self) -> None:
        committed = (
            Path(__file__).resolve().parents[2] / "fixtures/content/contract-speed.grf"
        ).read_bytes()
        self.assertEqual(grf_bytes(), committed)

    def test_rejects_truncated_packets(self) -> None:
        packet = info_packet()
        for length in range(len(packet)):
            with self.subTest(length=length), self.assertRaises(ContractError):
                parse_game_info(packet[:length])

    def test_rejects_trailing_bytes(self) -> None:
        packet = info_packet() + b"\x00"
        with self.assertRaises(ContractError):
            parse_game_info(packet)

    def test_rejects_unknown_schema(self) -> None:
        packet = bytearray(info_packet())
        packet[3] = 8
        with self.assertRaises(ContractError):
            parse_game_info(bytes(packet))

    def test_rejects_unterminated_server_name(self) -> None:
        packet = info_packet().replace(b"Contract server\x00", b"Contract serverX")
        with self.assertRaises(ContractError):
            parse_game_info(packet)


if __name__ == "__main__":
    unittest.main()
