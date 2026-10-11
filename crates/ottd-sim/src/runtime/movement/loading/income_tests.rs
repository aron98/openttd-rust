//! Synthetic arithmetic controls executed independently by pinned original functions.
use super::calculate;
use crate::content::CargoSpec;

#[test]
#[ignore = "requires INCOME_EXPECTED from independently compiled original arithmetic and INCOME_OUTPUT"]
fn original_fixed_point_time_distance_payment_matrix() -> Result<(), Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(std::env::var("INCOME_EXPECTED")?)?;
    let mut count_cases = 0;
    for line in text.lines() {
        let [count, distance, age, first, second, payment, expected]: [i64; 7] =
            serde_json::from_str(line)?;
        let spec = CargoSpec {
            label: 0,
            bitnum: 0,
            legend_colour: 0,
            rating_colour: 0,
            weight: 0,
            multiplier: 256,
            classes: 0,
            initial_payment: 0,
            transit_periods: [u8::try_from(first)?, u8::try_from(second)?],
            is_freight: false,
            town_acceptance_effect: 0,
            town_production_effect: 0,
            town_production_multiplier: 256,
            callback_mask: 0,
            current_payment: payment,
        };
        assert_eq!(
            calculate(
                &spec,
                u32::try_from(count)?,
                u32::try_from(distance)?,
                u16::try_from(age)?
            )?,
            expected,
            "{line}"
        );
        count_cases += 1;
    }
    assert_eq!(count_cases, 1296);
    std::fs::write(
        std::env::var("INCOME_OUTPUT")?,
        serde_json::to_vec_pretty(&serde_json::json!({
            "original_extracted_function_cases":count_cases,"synthetic":true,
            "includes":"time decay transitions, zero pieces/distance, UINT32 multiplication wrap, signed BigMulS argument/result narrowing"
        }))?,
    )?;
    Ok(())
}
