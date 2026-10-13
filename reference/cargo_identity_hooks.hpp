// SPDX-License-Identifier: GPL-2.0-only
#ifndef OTTD_REFERENCE_CARGO_IDENTITY_HOOKS_HPP
#define OTTD_REFERENCE_CARGO_IDENTITY_HOOKS_HPP
#include <exception>
class ByteReader;
class Engine;
void ReferenceCargoIdentityProperty(const char *, uint, uint, uint, int, size_t, int, bool);
void ReferenceCargoIdentityAllocated(uint, const Engine *, size_t);
void ReferenceCargoIdentityAfterSavedOverlay();
struct ReferenceCargoIdentityScope {
    uint feature, first, last;
    int property, result = -1, exceptions = std::uncaught_exceptions();
    ByteReader &reader;
    ReferenceCargoIdentityScope(uint feature, uint first, uint last, int property, ByteReader &reader);
    ~ReferenceCargoIdentityScope();
};
#endif
