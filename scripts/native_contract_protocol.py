# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Bounded, unauthenticated OpenTTD 15.3 game-info probe; no gameplay client."""

import socket
import struct
import time
from dataclasses import dataclass
from typing import Final

MAX_PACKET: Final = 16384
QUERY: Final = b"\x03\x00\x07"


class ContractError(RuntimeError):
    """A native baseline's explicit expected outcome was not observed."""


@dataclass(frozen=True, slots=True)
class GrfIdentity:
    grfid: str
    md5: str
    name: str


@dataclass(frozen=True, slots=True)
class GameInfo:
    schema: int
    ticks: int
    revision: str
    name: str
    width: int
    height: int
    landscape: int
    dedicated: bool
    calendar_date: int
    calendar_start: int
    companies: int
    grfs: tuple[GrfIdentity, ...]


class Reader:
    """Mutable bounded cursor over one complete game-info packet."""

    def __init__(self, data: bytes) -> None:
        self.data = data
        self.offset = 0

    def take(self, count: int) -> bytes:
        if self.offset + count > len(self.data):
            raise ContractError("Truncated game-info packet")
        result = self.data[self.offset : self.offset + count]
        self.offset += count
        return result

    def integer(self, size: int) -> int:
        return int.from_bytes(self.take(size), "little")

    def string(self, limit: int) -> str:
        end = self.data.find(b"\x00", self.offset, self.offset + limit)
        if end < 0:
            raise ContractError("Missing or overlong game-info string")
        raw = self.take(end - self.offset + 1)[:-1]
        try:
            return raw.decode("utf-8", errors="strict")
        except UnicodeDecodeError as error:
            raise ContractError("Invalid UTF-8 in game-info string") from error

    def boolean(self) -> bool:
        value = self.integer(1)
        if value > 1:
            raise ContractError("Invalid game-info boolean")
        return bool(value)


def parse_game_info(packet: bytes) -> GameInfo:
    """Accept only exact schema7 ID+MD5+name responses, including all framing bytes."""
    if not 4 <= len(packet) <= MAX_PACKET:
        raise ContractError("Game-info packet length outside bounds")
    reader = Reader(packet)
    if reader.integer(2) != len(packet) or reader.integer(1) != 6:
        raise ContractError("Game-info framing/type mismatch")
    schema = reader.integer(1)
    if schema != 7:
        raise ContractError("Unsupported game-info schema (expected 7)")
    ticks = reader.integer(8)
    if reader.integer(1) != 1:
        raise ContractError("Unsupported NewGRF serialization (expected ID+MD5+name)")
    script_version = reader.integer(4)
    script_name = reader.string(80)
    if script_version != 0xFFFFFFFF or script_name:
        raise ContractError("Unexpected GameScript in contract world")
    count = reader.integer(1)
    grfs = tuple(
        GrfIdentity(reader.take(4).hex(), reader.take(16).hex(), reader.string(80))
        for _ in range(count)
    )
    calendar_date, calendar_start = reader.integer(4), reader.integer(4)
    reader.integer(1)  # Maximum companies.
    companies = reader.integer(1)
    reader.integer(1)  # Legacy maximum spectators, now maximum clients.
    name, revision = reader.string(80), reader.string(33)
    reader.boolean()  # Password flag.
    reader.take(3)  # Maximum clients, current clients, spectators.
    width, height = reader.integer(2), reader.integer(2)
    landscape, dedicated = reader.integer(1), reader.boolean()
    if reader.offset != len(packet):
        raise ContractError("Trailing game-info bytes")
    return GameInfo(
        schema,
        ticks,
        revision,
        name,
        width,
        height,
        landscape,
        dedicated,
        calendar_date,
        calendar_start,
        companies,
        grfs,
    )


def receive_packet(connection: socket.socket) -> bytes:
    """Receive one bounded packet; the caller sets a finite socket timeout."""
    deadline = time.monotonic() + 2

    def receive_exact(count: int) -> bytes:
        data = bytearray()
        while len(data) < count:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise ContractError("Server packet receive deadline exceeded")
            connection.settimeout(remaining)
            fragment = connection.recv(count - len(data))
            if not fragment:
                raise ContractError("Server closed before a complete packet")
            data.extend(fragment)
        return bytes(data)

    prefix = receive_exact(2)
    size = int.from_bytes(prefix, "little")
    if not 3 <= size <= MAX_PACKET:
        raise ContractError("Server packet length outside bounds")
    return prefix + receive_exact(size - 2)


def query(port: int) -> bytes:
    with socket.create_connection(("127.0.0.1", port), timeout=2) as connection:
        connection.sendall(QUERY)
        return receive_packet(connection)


def wrong_revision(port: int) -> tuple[bytes, bytes]:
    """Exercise original pre-auth revision rejection, without authenticating."""
    payload = b"\x02contract-wrong-version\x00" + bytes(4)
    request = struct.pack("<H", len(payload) + 2) + payload
    with socket.create_connection(("127.0.0.1", port), timeout=2) as connection:
        connection.sendall(request)
        response = receive_packet(connection)
    if response != b"\x04\x00\x03\x08":
        raise ContractError(f"Expected WRONG_REVISION error, received {response.hex()}")
    return request, response
