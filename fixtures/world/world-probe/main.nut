/* SPDX-License-Identifier: GPL-2.0-only */
class WorldProbe extends AIController {
    state = null;
    function Check(ok, command) {
        if (!ok) throw command + ": " + AIError.GetLastErrorString();
        print("WORLD_COMMAND " + command + " true");
    }
    function Save() { return this.state; }
    function Load(version, data) {
        if (version != 1 || data == null || data.marker != 362) throw "Invalid world probe saved state";
        this.state = data;
    }
    function Start() {
        print("WORLD_SPEED engine=0 speed=" + AIEngine.GetMaxSpeed(0));
        if (this.state != null) {
            this.Check(AIVehicle.IsValidVehicle(this.state.vehicles[0]), "restored-first");
            this.Check(AIVehicle.IsValidVehicle(this.state.vehicles[1]), "restored-second");
            this.Check(!AIVehicle.IsValidVehicle(this.state.discarded), "restored-hole");
            print("WORLD_RESTORED true");
            while (true) this.Sleep(100);
        }
        this.Check(AICompany.SetName("World Probe"), "company-name");
        this.Check(AICompany.SetLoanAmount(AICompany.GetMaxLoanAmount()), "company-loan");
        local engines = AIEngineList(AIVehicle.VT_ROAD);
        engines.Valuate(AIEngine.IsBuildable);
        engines.KeepValue(1);
        local engine = engines.Begin();
        this.Check(AIEngine.IsValidEngine(engine), "select-road-engine");
        AIRoad.SetCurrentRoadType(AIEngine.GetRoadType(engine));
        local depot = AIMap.TILE_INVALID;
        {
            local test = AITestMode();
            for (local tile = 1000; tile < AIMap.GetMapSize() - AIMap.GetMapSizeX() - 1; tile++) {
                if (AITile.IsBuildable(tile) && AITile.GetSlope(tile) == AITile.SLOPE_FLAT && AIRoad.BuildRoadDepot(tile, tile + 1)) {
                    depot = tile;
                    break;
                }
            }
        }
        this.Check(AIMap.IsValidTile(depot), "find-depot");
        this.Check(AIRoad.BuildRoadDepot(depot, depot + 1), "build-depot");
        local first = AIVehicle.BuildVehicle(depot, engine);
        this.Check(AIVehicle.IsValidVehicle(first), "build-first");
        local discarded = AIVehicle.BuildVehicle(depot, engine);
        this.Check(AIVehicle.IsValidVehicle(discarded), "build-discarded");
        local second = AIVehicle.BuildVehicle(depot, engine);
        this.Check(AIVehicle.IsValidVehicle(second), "build-second");
        this.Check(AIVehicle.SetName(first, "World First"), "vehicle-name");
        this.Check(AIOrder.AppendOrder(first, depot, AIOrder.OF_SERVICE_IF_NEEDED), "depot-order");
        this.Check(AIOrder.ShareOrders(second, first), "shared-orders");
        local parent_group = AIGroup.CreateGroup(AIVehicle.VT_ROAD, AIGroup.GROUP_INVALID);
        this.Check(AIGroup.IsValidGroup(parent_group), "parent-group");
        local child = AIGroup.CreateGroup(AIVehicle.VT_ROAD, parent_group);
        this.Check(AIGroup.IsValidGroup(child), "child-group");
        this.Check(AIGroup.MoveVehicle(parent_group, first), "parent-vehicle");
        this.Check(AIGroup.MoveVehicle(child, second), "child-vehicle");
        this.Check(AIVehicle.SellVehicle(discarded), "sell-middle");
        this.state = { marker = 362, vehicles = [first, second], discarded = discarded, depot = depot,
            groups = [parent_group, child], nested = { signed = -7, truth = true, nothing = null } };
        print("WORLD_READY first=" + first + " second=" + second + " hole=" + discarded);
        while (true) this.Sleep(100);
    }
}
