use serde::Serialize;
/// Native price indices; discriminants follow the pinned `NewGRF` price order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[repr(u8)]
pub enum Price {
    /// Native `PR_STATION_VALUE` base.
    StationValue = 0,
    /// Native `PR_BUILD_RAIL` base.
    BuildRail = 1,
    /// Native `PR_BUILD_ROAD` base.
    BuildRoad = 2,
    /// Native `PR_BUILD_SIGNALS` base.
    BuildSignals = 3,
    /// Native `PR_BUILD_BRIDGE` base.
    BuildBridge = 4,
    /// Native `PR_BUILD_DEPOT_TRAIN` base.
    BuildDepotTrain = 5,
    /// Native `PR_BUILD_DEPOT_ROAD` base.
    BuildDepotRoad = 6,
    /// Native `PR_BUILD_DEPOT_SHIP` base.
    BuildDepotShip = 7,
    /// Native `PR_BUILD_TUNNEL` base.
    BuildTunnel = 8,
    /// Native `PR_BUILD_STATION_RAIL` base.
    BuildStationRail = 9,
    /// Native `PR_BUILD_STATION_RAIL_LENGTH` base.
    BuildStationRailLength = 10,
    /// Native `PR_BUILD_STATION_AIRPORT` base.
    BuildStationAirport = 11,
    /// Native `PR_BUILD_STATION_BUS` base.
    BuildStationBus = 12,
    /// Native `PR_BUILD_STATION_TRUCK` base.
    BuildStationTruck = 13,
    /// Native `PR_BUILD_STATION_DOCK` base.
    BuildStationDock = 14,
    /// Native `PR_BUILD_VEHICLE_TRAIN` base.
    BuildVehicleTrain = 15,
    /// Native `PR_BUILD_VEHICLE_WAGON` base.
    BuildVehicleWagon = 16,
    /// Native `PR_BUILD_VEHICLE_AIRCRAFT` base.
    BuildVehicleAircraft = 17,
    /// Native `PR_BUILD_VEHICLE_ROAD` base.
    BuildVehicleRoad = 18,
    /// Native `PR_BUILD_VEHICLE_SHIP` base.
    BuildVehicleShip = 19,
    /// Native `PR_BUILD_TREES` base.
    BuildTrees = 20,
    /// Native `PR_TERRAFORM` base.
    Terraform = 21,
    /// Native `PR_CLEAR_GRASS` base.
    ClearGrass = 22,
    /// Native `PR_CLEAR_ROUGH` base.
    ClearRough = 23,
    /// Native `PR_CLEAR_ROCKS` base.
    ClearRocks = 24,
    /// Native `PR_CLEAR_FIELDS` base.
    ClearFields = 25,
    /// Native `PR_CLEAR_TREES` base.
    ClearTrees = 26,
    /// Native `PR_CLEAR_RAIL` base.
    ClearRail = 27,
    /// Native `PR_CLEAR_SIGNALS` base.
    ClearSignals = 28,
    /// Native `PR_CLEAR_BRIDGE` base.
    ClearBridge = 29,
    /// Native `PR_CLEAR_DEPOT_TRAIN` base.
    ClearDepotTrain = 30,
    /// Native `PR_CLEAR_DEPOT_ROAD` base.
    ClearDepotRoad = 31,
    /// Native `PR_CLEAR_DEPOT_SHIP` base.
    ClearDepotShip = 32,
    /// Native `PR_CLEAR_TUNNEL` base.
    ClearTunnel = 33,
    /// Native `PR_CLEAR_WATER` base.
    ClearWater = 34,
    /// Native `PR_CLEAR_STATION_RAIL` base.
    ClearStationRail = 35,
    /// Native `PR_CLEAR_STATION_AIRPORT` base.
    ClearStationAirport = 36,
    /// Native `PR_CLEAR_STATION_BUS` base.
    ClearStationBus = 37,
    /// Native `PR_CLEAR_STATION_TRUCK` base.
    ClearStationTruck = 38,
    /// Native `PR_CLEAR_STATION_DOCK` base.
    ClearStationDock = 39,
    /// Native `PR_CLEAR_HOUSE` base.
    ClearHouse = 40,
    /// Native `PR_CLEAR_ROAD` base.
    ClearRoad = 41,
    /// Native `PR_RUNNING_TRAIN_STEAM` base.
    RunningTrainSteam = 42,
    /// Native `PR_RUNNING_TRAIN_DIESEL` base.
    RunningTrainDiesel = 43,
    /// Native `PR_RUNNING_TRAIN_ELECTRIC` base.
    RunningTrainElectric = 44,
    /// Native `PR_RUNNING_AIRCRAFT` base.
    RunningAircraft = 45,
    /// Native `PR_RUNNING_ROADVEH` base.
    RunningRoadveh = 46,
    /// Native `PR_RUNNING_SHIP` base.
    RunningShip = 47,
    /// Native `PR_BUILD_INDUSTRY` base.
    BuildIndustry = 48,
    /// Native `PR_CLEAR_INDUSTRY` base.
    ClearIndustry = 49,
    /// Native `PR_BUILD_OBJECT` base.
    BuildObject = 50,
    /// Native `PR_CLEAR_OBJECT` base.
    ClearObject = 51,
    /// Native `PR_BUILD_WAYPOINT_RAIL` base.
    BuildWaypointRail = 52,
    /// Native `PR_CLEAR_WAYPOINT_RAIL` base.
    ClearWaypointRail = 53,
    /// Native `PR_BUILD_WAYPOINT_BUOY` base.
    BuildWaypointBuoy = 54,
    /// Native `PR_CLEAR_WAYPOINT_BUOY` base.
    ClearWaypointBuoy = 55,
    /// Native `PR_TOWN_ACTION` base.
    TownAction = 56,
    /// Native `PR_BUILD_FOUNDATION` base.
    BuildFoundation = 57,
    /// Native `PR_BUILD_INDUSTRY_RAW` base.
    BuildIndustryRaw = 58,
    /// Native `PR_BUILD_TOWN` base.
    BuildTown = 59,
    /// Native `PR_BUILD_CANAL` base.
    BuildCanal = 60,
    /// Native `PR_CLEAR_CANAL` base.
    ClearCanal = 61,
    /// Native `PR_BUILD_AQUEDUCT` base.
    BuildAqueduct = 62,
    /// Native `PR_CLEAR_AQUEDUCT` base.
    ClearAqueduct = 63,
    /// Native `PR_BUILD_LOCK` base.
    BuildLock = 64,
    /// Native `PR_CLEAR_LOCK` base.
    ClearLock = 65,
    /// Native `PR_INFRASTRUCTURE_RAIL` base.
    InfrastructureRail = 66,
    /// Native `PR_INFRASTRUCTURE_ROAD` base.
    InfrastructureRoad = 67,
    /// Native `PR_INFRASTRUCTURE_WATER` base.
    InfrastructureWater = 68,
    /// Native `PR_INFRASTRUCTURE_STATION` base.
    InfrastructureStation = 69,
    /// Native `PR_INFRASTRUCTURE_AIRPORT` base.
    InfrastructureAirport = 70,
}
impl Price {
    /// Every native price, in native order.
    pub const ALL: [Self; 71] = [
        Self::StationValue,
        Self::BuildRail,
        Self::BuildRoad,
        Self::BuildSignals,
        Self::BuildBridge,
        Self::BuildDepotTrain,
        Self::BuildDepotRoad,
        Self::BuildDepotShip,
        Self::BuildTunnel,
        Self::BuildStationRail,
        Self::BuildStationRailLength,
        Self::BuildStationAirport,
        Self::BuildStationBus,
        Self::BuildStationTruck,
        Self::BuildStationDock,
        Self::BuildVehicleTrain,
        Self::BuildVehicleWagon,
        Self::BuildVehicleAircraft,
        Self::BuildVehicleRoad,
        Self::BuildVehicleShip,
        Self::BuildTrees,
        Self::Terraform,
        Self::ClearGrass,
        Self::ClearRough,
        Self::ClearRocks,
        Self::ClearFields,
        Self::ClearTrees,
        Self::ClearRail,
        Self::ClearSignals,
        Self::ClearBridge,
        Self::ClearDepotTrain,
        Self::ClearDepotRoad,
        Self::ClearDepotShip,
        Self::ClearTunnel,
        Self::ClearWater,
        Self::ClearStationRail,
        Self::ClearStationAirport,
        Self::ClearStationBus,
        Self::ClearStationTruck,
        Self::ClearStationDock,
        Self::ClearHouse,
        Self::ClearRoad,
        Self::RunningTrainSteam,
        Self::RunningTrainDiesel,
        Self::RunningTrainElectric,
        Self::RunningAircraft,
        Self::RunningRoadveh,
        Self::RunningShip,
        Self::BuildIndustry,
        Self::ClearIndustry,
        Self::BuildObject,
        Self::ClearObject,
        Self::BuildWaypointRail,
        Self::ClearWaypointRail,
        Self::BuildWaypointBuoy,
        Self::ClearWaypointBuoy,
        Self::TownAction,
        Self::BuildFoundation,
        Self::BuildIndustryRaw,
        Self::BuildTown,
        Self::BuildCanal,
        Self::ClearCanal,
        Self::BuildAqueduct,
        Self::ClearAqueduct,
        Self::BuildLock,
        Self::ClearLock,
        Self::InfrastructureRail,
        Self::InfrastructureRoad,
        Self::InfrastructureWater,
        Self::InfrastructureStation,
        Self::InfrastructureAirport,
    ];
}
