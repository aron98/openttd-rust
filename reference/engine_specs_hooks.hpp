// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_ENGINE_SPECS_HOOKS_HPP
#define OTTD_REFERENCE_ENGINE_SPECS_HOOKS_HPP
#include <exception>
class ByteReader;
class Engine;
void ReferenceEngineSpecsRoad(const char *phase, uint first, uint last, int property, size_t remaining, int result, bool unwinding);
void ReferenceEngineSpecsAllocated(uint local_id, const Engine *engine, size_t remaining);
struct ReferenceEngineSpecsRoadScope {
    uint first, last;
    int property, result = -1, exceptions = std::uncaught_exceptions();
    ByteReader &reader;
    ReferenceEngineSpecsRoadScope(uint first, uint last, int property, ByteReader &reader);
    ~ReferenceEngineSpecsRoadScope();
};
#endif
