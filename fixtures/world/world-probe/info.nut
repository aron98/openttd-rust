/* SPDX-License-Identifier: GPL-2.0-only */
class WorldProbeInfo extends AIInfo {
    function GetAuthor()      { return "openttd-rust contributors"; }
    function GetName()        { return "WorldProbe"; }
    function GetShortName()   { return "WRLD"; }
    function GetDescription() { return "Native stage 2 object graph fixture builder."; }
    function GetVersion()     { return 1; }
    function GetAPIVersion()  { return "15"; }
    function GetDate()        { return "2026-10-09"; }
    function CreateInstance() { return "WorldProbe"; }
    function UseAsRandomAI()  { return false; }
}
RegisterAI(WorldProbeInfo());
