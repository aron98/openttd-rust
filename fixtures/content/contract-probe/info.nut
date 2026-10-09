/* SPDX-License-Identifier: GPL-2.0-only */
class ContractProbeInfo extends AIInfo {
    function GetAuthor()      { return "openttd-rust contributors"; }
    function GetName()        { return "ContractProbe"; }
    function GetShortName()   { return "CTRP"; }
    function GetDescription() { return "Native-only contract property and command observer."; }
    function GetVersion()     { return 1; }
    function GetAPIVersion()  { return "15"; }
    function GetDate()        { return "2026-10-08"; }
    function CreateInstance() { return "ContractProbe"; }
    function UseAsRandomAI()  { return false; }
}
RegisterAI(ContractProbeInfo());
