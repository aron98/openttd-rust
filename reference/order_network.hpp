#ifndef OTTD_REFERENCE_ORDER_NETWORK_HPP
#define OTTD_REFERENCE_ORDER_NETWORK_HPP
#include <filesystem>
#include <fstream>
#include <cstdio>
#include <cstdlib>

namespace ReferenceOrderNetwork {
[[noreturn]] inline void HostError(const char *message)
{
    fmt::print(stderr, "Order network reference host error: {}\n", message);
    std::fflush(stderr);
    std::_Exit(1);
}
template <typename Bytes> inline void Capture(const Bytes &bytes)
{
    const char *path = std::getenv("OTTD_ORDER_NETWORK_INPUT_PATH");
    if (path == nullptr) return;
    if (std::filesystem::exists(path)) HostError("input capture already exists");
    if (bytes.empty()) HostError("input capture is empty");
    std::ofstream stream(path, std::ios::binary);
    if (!stream.good()) HostError("cannot open input capture");
    for (uint8_t byte : bytes) stream.put(static_cast<char>(byte));
    stream.close();
    if (!stream.good()) HostError("cannot write complete input capture");
}
}
#endif
