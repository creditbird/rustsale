use crate::google_maps_jobs::types::{ScrapeErrorKind, ScrapeJobError};
use crate::scrapers::grid::{
    bbox_dimensions_km, count_initial_cells, recommended_step_for_cap, BoundingBox, GridLimits,
    MAX_GRID_STEP_KM, MIN_GRID_STEP_KM,
};
use serde::{Deserialize, Serialize};

pub type ScrapeError = ScrapeJobError;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GridEstimate {
    pub initial_cells: usize,
    pub recommended_min_step_km: Option<f64>,
    pub width_km: f64,
    pub height_km: f64,
}

pub fn validate_and_estimate_grid(
    bbox: BoundingBox,
    step_km: f64,
    limits: GridLimits,
) -> Result<GridEstimate, ScrapeError> {
    validate_bbox(bbox)?;
    if !step_km.is_finite() || step_km <= 0.0 {
        return Err(invalid_input(
            "step_km must be finite and greater than zero",
        ));
    }
    if !(MIN_GRID_STEP_KM..=MAX_GRID_STEP_KM).contains(&step_km) {
        return Err(invalid_input(format!(
            "step_km must be between {MIN_GRID_STEP_KM} and {MAX_GRID_STEP_KM} km"
        )));
    }
    if limits.max_initial_cells == 0 || limits.max_total_cells == 0 {
        return Err(invalid_input("grid cell limits must be greater than zero"));
    }
    if limits.max_initial_cells > limits.max_total_cells {
        return Err(invalid_input(
            "max_initial_cells cannot exceed max_total_cells",
        ));
    }
    if !limits.min_cell_size_m.is_finite() || limits.min_cell_size_m <= 0.0 {
        return Err(invalid_input(
            "min_cell_size_m must be finite and greater than zero",
        ));
    }

    let (width_km, height_km) = bbox_dimensions_km(bbox);
    let initial_cells = count_initial_cells(bbox, step_km);
    let recommended_min_step_km = if initial_cells > limits.max_initial_cells {
        Some(recommended_step_for_cap(
            bbox,
            step_km,
            limits.max_initial_cells,
        ))
    } else {
        None
    };

    Ok(GridEstimate {
        initial_cells,
        recommended_min_step_km,
        width_km,
        height_km,
    })
}

fn validate_bbox(bbox: BoundingBox) -> Result<(), ScrapeError> {
    let coordinates = [bbox.lat_min, bbox.lat_max, bbox.lng_min, bbox.lng_max];
    if coordinates.iter().any(|value| !value.is_finite()) {
        return Err(invalid_input("bounding box coordinates must be finite"));
    }
    if !(-90.0..=90.0).contains(&bbox.lat_min)
        || !(-90.0..=90.0).contains(&bbox.lat_max)
        || !(-180.0..=180.0).contains(&bbox.lng_min)
        || !(-180.0..=180.0).contains(&bbox.lng_max)
    {
        return Err(invalid_input("bounding box coordinates are out of range"));
    }
    if bbox.lat_min >= bbox.lat_max || bbox.lng_min >= bbox.lng_max {
        return Err(invalid_input(
            "bounding box minimums must be less than maximums",
        ));
    }
    Ok(())
}

fn invalid_input(message: impl Into<String>) -> ScrapeError {
    ScrapeJobError {
        kind: ScrapeErrorKind::InvalidInput,
        message: message.into(),
        retryable: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::google_maps_jobs::types::ScrapeErrorKind;
    use crate::scrapers::grid::{BoundingBox, GridLimits};

    fn limits(max_initial_cells: usize) -> GridLimits {
        GridLimits {
            max_depth: 6,
            min_cell_size_m: 20.0,
            max_initial_cells,
            max_total_cells: 25_000,
        }
    }

    #[test]
    fn rejects_zero_negative_nan_and_infinite_step() {
        let bbox = BoundingBox {
            lat_min: 10.0,
            lat_max: 10.5,
            lng_min: 106.0,
            lng_max: 106.5,
        };

        for step_km in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let error = validate_and_estimate_grid(bbox, step_km, limits(10_000)).unwrap_err();
            assert_eq!(error.kind, ScrapeErrorKind::InvalidInput);
        }
    }

    #[test]
    fn rejects_step_outside_supported_range() {
        let bbox = BoundingBox {
            lat_min: 10.0,
            lat_max: 10.5,
            lng_min: 106.0,
            lng_max: 106.5,
        };

        for step_km in [0.05, 0.19, 50.5, 150.0] {
            let error = validate_and_estimate_grid(bbox, step_km, limits(10_000)).unwrap_err();
            assert_eq!(error.kind, ScrapeErrorKind::InvalidInput, "step {step_km}");
        }
        for step_km in [0.2, 1.0, 50.0] {
            assert!(
                validate_and_estimate_grid(bbox, step_km, limits(10_000)).is_ok(),
                "step {step_km}"
            );
        }
    }

    #[test]
    fn rejects_reversed_or_out_of_range_bbox() {
        for bbox in [
            BoundingBox {
                lat_min: 11.0,
                lat_max: 10.0,
                lng_min: 106.0,
                lng_max: 107.0,
            },
            BoundingBox {
                lat_min: -91.0,
                lat_max: 10.0,
                lng_min: 106.0,
                lng_max: 107.0,
            },
            BoundingBox {
                lat_min: 10.0,
                lat_max: 11.0,
                lng_min: 106.0,
                lng_max: 181.0,
            },
            BoundingBox {
                lat_min: f64::NAN,
                lat_max: 11.0,
                lng_min: 106.0,
                lng_max: 107.0,
            },
        ] {
            let error = validate_and_estimate_grid(bbox, 2.0, limits(10_000)).unwrap_err();
            assert_eq!(error.kind, ScrapeErrorKind::InvalidInput);
        }
    }

    #[test]
    fn returns_recommended_step_when_initial_cell_cap_is_exceeded() {
        let estimate = validate_and_estimate_grid(
            BoundingBox {
                lat_min: 10.0,
                lat_max: 11.0,
                lng_min: 106.0,
                lng_max: 107.0,
            },
            1.0,
            limits(100),
        )
        .unwrap();

        assert!(estimate.initial_cells > 100);
        assert!(estimate.recommended_min_step_km.unwrap() > 1.0);
        assert!(estimate.width_km > 0.0);
        assert!(estimate.height_km > 0.0);
    }
}
