// Required original capture boundary for the formerly refused natural disaster horizon.
use ottd_sim::runtime::SimulationRuntime;
use serde_json::{Value, json};
use std::{
    fmt, fs,
    path::Path,
};
include!("../vehicle_year_support/groups.rs");
include!("../company_rating_support/ratings.rs");

#[derive(Debug)]
struct RequiredDisasterCapture(String);
impl fmt::Display for RequiredDisasterCapture {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "required genuine 60680-call disaster capture: {}",
            self.0
        )
    }
}
impl std::error::Error for RequiredDisasterCapture {}
struct DisasterCapture {
    native: PathBuf,
    output: PathBuf,
}
impl DisasterCapture {
    fn required(input: &World) -> Result<Self> {
        let native = std::env::var("INDUSTRY_RECIPIENT_NATIVE")
            .map(PathBuf::from)
            .map_err(|_| RequiredDisasterCapture("INDUSTRY_RECIPIENT_NATIVE is missing".into()))?;
        for label in ["initial", "prefix", "final"] {
            for suffix in [
                "sav",
                "world.json",
                "derived.json",
                "runtime.json",
                "movement.json",
                "groups.json",
                "rating.json",
            ] {
                let path = native.join(format!("{label}.{suffix}"));
                if !path.is_file() {
                    return Err(RequiredDisasterCapture(path.display().to_string()).into());
                }
            }
        }
        let original: Value =
            serde_json::from_slice(&fs::read(native.join("initial.world.json"))?)?;
        assert_eq!(
            input.saved_json()?,
            original,
            "unchanged authentic original input"
        );
        let output = PathBuf::from(std::env::var("DISASTER_OUTPUT")?);
        fs::create_dir_all(&output)?;
        Ok(Self { native, output })
    }
    fn compare(&self, runtime: &SimulationRuntime, label: &str) -> Result {
        native_views::compare(runtime, &self.output, label)?;
        let groups: Value =
            serde_json::from_slice(&fs::read(self.native.join(format!("{label}.groups.json")))?)?;
        assert_eq!(
            serde_json::to_value(runtime.group_profit_caches())?,
            owned_native(&groups)?
        );
        fs::write(
            self.output.join(format!("{label}.ratings.json")),
            serde_json::to_vec(runtime.company_rating_caches())?,
        )?;
        compare_raw_ratings(runtime, &self.native, label)
    }
    fn finish(&self, runtime: &SimulationRuntime, prefix: u32) -> Result {
        self.compare(runtime, "final")?;
        fs::write(
            self.output.join("PASS.json"),
            serde_json::to_vec_pretty(&json!({
                "calls":60680,"positive_prefix":prefix,"genuine_native":true,
                "world_derived_nine_views_three_group_fields_all_150_ratings":true,
                "historical_disaster_refusal_supported":true,"rollback_credit":false
            }))?,
        )?;
        Ok(())
    }
}
