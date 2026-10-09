/* SPDX-License-Identifier: GPL-2.0-only */
class WorldGameInfo extends GSInfo {
    function GetAuthor() { return "openttd-rust contributors"; }
    function GetName() { return "WorldGame"; }
    function GetShortName() { return "WRGS"; }
    function GetDescription() { return "Native saved GameScript, goal, story and league fixture."; }
    function GetVersion() { return 1; }
    function GetAPIVersion() { return "15"; }
    function GetDate() { return "2026-10-09"; }
    function CreateInstance() { return "WorldGame"; }
}
RegisterGS(WorldGameInfo());
