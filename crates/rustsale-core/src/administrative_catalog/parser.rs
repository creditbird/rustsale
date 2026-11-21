use std::{
    collections::{HashMap, HashSet},
    io::BufRead,
};

use anyhow::{anyhow, Context, Result};
use chrono::NaiveDate;

use super::types::{
    AdministrativeArea, AdministrativeAreaKind, AdministrativeCountry, ParsedCountryData,
};

const MAX_SOURCE_ROW_BYTES: usize = 64 * 1024;
const MAX_RETAINED_AREAS: usize = 2_000_000;

#[derive(Debug)]
struct ParsedRow {
    area: AdministrativeArea,
    admin_codes: [String; 5],
}

pub fn parse_admin5_codes<R: BufRead>(reader: R) -> Result<HashMap<i64, String>> {
    let mut codes = HashMap::new();
    for (index, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("failed reading ADM5 row {}", index + 1))?;
        if line.len() > MAX_SOURCE_ROW_BYTES {
            return Err(anyhow!("ADM5 row {} exceeds the size limit", index + 1));
        }
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let mut fields = line.split('\t');
        let geoname_id = fields
            .next()
            .ok_or_else(|| anyhow!("ADM5 row {} is missing a GeoNames ID", index + 1))?
            .trim()
            .parse::<i64>()
            .with_context(|| format!("invalid ADM5 GeoNames ID on row {}", index + 1))?;
        let code = fields
            .next()
            .ok_or_else(|| anyhow!("ADM5 row {} is missing a code", index + 1))?
            .trim();
        if code.is_empty() || fields.next().is_some() {
            return Err(anyhow!("invalid ADM5 row {}", index + 1));
        }
        if codes.insert(geoname_id, code.to_string()).is_some() {
            return Err(anyhow!("duplicate ADM5 GeoNames ID {geoname_id}"));
        }
    }
    Ok(codes)
}

pub fn parse_country_dump<R: BufRead>(
    country: &AdministrativeCountry,
    reader: R,
    admin5_codes: &HashMap<i64, String>,
) -> Result<ParsedCountryData> {
    let mut rows = Vec::new();
    let mut source_updated_at: Option<String> = None;
    let mut bbox: Option<[f64; 4]> = None;

    for (index, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("failed reading GeoNames row {}", index + 1))?;
        if line.len() > MAX_SOURCE_ROW_BYTES {
            return Err(anyhow!("GeoNames row {} exceeds the size limit", index + 1));
        }
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 19 {
            return Err(anyhow!(
                "GeoNames row {} has {} columns, expected 19",
                index + 1,
                fields.len()
            ));
        }
        let feature_class = fields[6].trim();
        let feature_code = fields[7].trim();
        let retained = retained_kind(feature_class, feature_code);
        let Some((kind, administrative_level)) = retained else {
            continue;
        };

        if rows.len() >= MAX_RETAINED_AREAS {
            return Err(anyhow!(
                "country {} exceeds the {} retained-area limit",
                country.code,
                MAX_RETAINED_AREAS
            ));
        }
        let geoname_id = parse_required::<i64>(fields[0], index, "GeoNames ID")?;
        let country_code = fields[8].trim().to_ascii_uppercase();
        if country_code != country.code {
            return Err(anyhow!(
                "GeoNames row {} belongs to {country_code}, expected {}",
                index + 1,
                country.code
            ));
        }
        let name = fields[1].trim();
        if name.is_empty() {
            return Err(anyhow!("GeoNames row {} has an empty name", index + 1));
        }
        let latitude = parse_required::<f64>(fields[4], index, "latitude")?;
        let longitude = parse_required::<f64>(fields[5], index, "longitude")?;
        if !latitude.is_finite() || !(-90.0..=90.0).contains(&latitude) {
            return Err(anyhow!("invalid latitude on GeoNames row {}", index + 1));
        }
        if !longitude.is_finite() || !(-180.0..=180.0).contains(&longitude) {
            return Err(anyhow!("invalid longitude on GeoNames row {}", index + 1));
        }
        let population = if fields[14].trim().is_empty() {
            0
        } else {
            parse_required::<u64>(fields[14], index, "population")?
        };
        let modification_date = fields[18].trim();
        NaiveDate::parse_from_str(modification_date, "%Y-%m-%d")
            .with_context(|| format!("invalid modification date on GeoNames row {}", index + 1))?;
        if source_updated_at
            .as_deref()
            .is_none_or(|current| modification_date > current)
        {
            source_updated_at = Some(modification_date.to_string());
        }
        update_bbox(&mut bbox, latitude, longitude);

        let mut admin_codes = [
            fields[10].trim().to_string(),
            fields[11].trim().to_string(),
            fields[12].trim().to_string(),
            fields[13].trim().to_string(),
            String::new(),
        ];
        if let Some(admin5_code) = admin5_codes.get(&geoname_id) {
            admin_codes[4] = admin5_code.clone();
        }
        rows.push(ParsedRow {
            area: AdministrativeArea {
                geoname_id,
                country_code,
                parent_id: None,
                name: name.to_string(),
                ascii_name: fields[2].trim().to_string(),
                alternate_names: fields[3].trim().to_string(),
                feature_code: feature_code.to_string(),
                administrative_level,
                kind,
                latitude,
                longitude,
                population,
                modification_date: modification_date.to_string(),
                canonical_path: String::new(),
            },
            admin_codes,
        });
    }

    resolve_parents(&mut rows);
    resolve_canonical_paths(country, &mut rows)?;
    let mut areas: Vec<_> = rows.into_iter().map(|row| row.area).collect();
    areas.sort_by(|left, right| {
        left.canonical_path
            .cmp(&right.canonical_path)
            .then_with(|| left.geoname_id.cmp(&right.geoname_id))
    });
    Ok(ParsedCountryData {
        country_code: country.code.clone(),
        areas,
        source_updated_at,
        bbox,
    })
}

fn retained_kind(
    feature_class: &str,
    feature_code: &str,
) -> Option<(AdministrativeAreaKind, Option<u8>)> {
    if feature_class == "A" {
        let level = match feature_code {
            "ADM1" => 1,
            "ADM2" => 2,
            "ADM3" => 3,
            "ADM4" => 4,
            "ADM5" => 5,
            _ => return None,
        };
        return Some((AdministrativeAreaKind::Administrative, Some(level)));
    }
    if feature_class == "P"
        && feature_code.starts_with("PPL")
        && !matches!(feature_code, "PPLCH" | "PPLH" | "PPLQ" | "PPLW" | "PPLX")
    {
        return Some((AdministrativeAreaKind::PopulatedPlace, None));
    }
    None
}

fn parse_required<T>(value: &str, zero_based_index: usize, field: &str) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display + Send + Sync + 'static,
{
    value.trim().parse::<T>().map_err(|error| {
        anyhow!(
            "invalid {field} on GeoNames row {}: {error}",
            zero_based_index + 1
        )
    })
}

fn update_bbox(bbox: &mut Option<[f64; 4]>, latitude: f64, longitude: f64) {
    match bbox {
        Some([min_lat, max_lat, min_lng, max_lng]) => {
            *min_lat = min_lat.min(latitude);
            *max_lat = max_lat.max(latitude);
            *min_lng = min_lng.min(longitude);
            *max_lng = max_lng.max(longitude);
        }
        None => *bbox = Some([latitude, latitude, longitude, longitude]),
    }
}

fn resolve_parents(rows: &mut [ParsedRow]) {
    let mut admin_index = HashMap::new();
    for row in rows.iter() {
        if let Some(level) = row.area.administrative_level {
            if let Some(key) = hierarchy_key(&row.admin_codes, level) {
                admin_index.entry(key).or_insert(row.area.geoname_id);
            }
        }
    }

    for row in rows.iter_mut() {
        let deepest_parent_level = row
            .area
            .administrative_level
            .map_or(5, |level| level.saturating_sub(1));
        row.area.parent_id = (1..=deepest_parent_level)
            .rev()
            .filter_map(|level| hierarchy_key(&row.admin_codes, level))
            .find_map(|key| admin_index.get(&key).copied())
            .filter(|parent_id| *parent_id != row.area.geoname_id);
    }
}

fn hierarchy_key(codes: &[String; 5], level: u8) -> Option<String> {
    let level = usize::from(level);
    if level == 0 || level > codes.len() || codes[..level].iter().any(String::is_empty) {
        return None;
    }
    Some(codes[..level].join("\u{1f}"))
}

fn resolve_canonical_paths(country: &AdministrativeCountry, rows: &mut [ParsedRow]) -> Result<()> {
    let ancestry: HashMap<i64, (String, Option<i64>)> = rows
        .iter()
        .map(|row| {
            (
                row.area.geoname_id,
                (row.area.name.clone(), row.area.parent_id),
            )
        })
        .collect();
    let mut memo = HashMap::new();
    for row in rows.iter_mut() {
        row.area.canonical_path = canonical_path(
            row.area.geoname_id,
            &country.name,
            &ancestry,
            &mut memo,
            &mut HashSet::new(),
        )?;
    }
    Ok(())
}

fn canonical_path(
    geoname_id: i64,
    country_name: &str,
    ancestry: &HashMap<i64, (String, Option<i64>)>,
    memo: &mut HashMap<i64, String>,
    visiting: &mut HashSet<i64>,
) -> Result<String> {
    if let Some(path) = memo.get(&geoname_id) {
        return Ok(path.clone());
    }
    if !visiting.insert(geoname_id) {
        return Err(anyhow!("administrative hierarchy cycle at {geoname_id}"));
    }
    let (name, parent_id) = ancestry
        .get(&geoname_id)
        .ok_or_else(|| anyhow!("missing area {geoname_id} while building its path"))?;
    let path = if let Some(parent_id) = parent_id {
        format!(
            "{name}, {}",
            canonical_path(*parent_id, country_name, ancestry, memo, visiting)?
        )
    } else {
        format!("{name}, {country_name}")
    };
    visiting.remove(&geoname_id);
    memo.insert(geoname_id, path.clone());
    Ok(path)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{parse_admin5_codes, parse_country_dump};
    use crate::administrative_catalog::{
        countries::bundled_countries, types::AdministrativeAreaKind,
    };

    const DUMP: &str = concat!(
        "100\tAlpha Province\tAlpha Province\tProvince Alpha\t10\t20\tA\tADM1\tVN\t\t01\t\t\t\t0\t\t0\tAsia/Ho_Chi_Minh\t2026-08-01\n",
        "200\tBeta District\tBeta District\tDistrict Beta\t11\t21\tA\tADM2\tVN\t\t01\t02\t\t\t0\t\t0\tAsia/Ho_Chi_Minh\t2026-08-02\n",
        "300\tGamma Commune\tGamma Commune\tCommune Gamma\t12\t22\tA\tADM3\tVN\t\t01\t02\t03\t\t0\t\t0\tAsia/Ho_Chi_Minh\t2026-08-03\n",
        "400\tDelta Ward\tDelta Ward\tWard Delta\t13\t23\tA\tADM4\tVN\t\t01\t02\t03\t04\t0\t\t0\tAsia/Ho_Chi_Minh\t2026-08-04\n",
        "500\tEpsilon Hamlet\tEpsilon Hamlet\tHamlet Epsilon\t14\t24\tA\tADM5\tVN\t\t01\t02\t03\t04\t0\t\t0\tAsia/Ho_Chi_Minh\t2026-08-05\n",
        "600\tZeta City\tZeta City\tCity Zeta,Zeta\t15\t25\tP\tPPLA3\tVN\t\t01\t02\t03\t04\t5000\t\t0\tAsia/Ho_Chi_Minh\t2026-08-06\n",
        "700\tOld District\tOld District\t\t16\t26\tA\tADM2H\tVN\t\t01\t07\t\t\t0\t\t0\tAsia/Ho_Chi_Minh\t2020-01-01\n",
        "800\tAbandoned Village\tAbandoned Village\t\t17\t27\tP\tPPLQ\tVN\t\t01\t02\t03\t04\t0\t\t0\tAsia/Ho_Chi_Minh\t2020-01-01\n",
        "900\tVillage Section\tVillage Section\t\t18\t28\tP\tPPLX\tVN\t\t01\t02\t03\t04\t0\t\t0\tAsia/Ho_Chi_Minh\t2020-01-01\n",
    );

    #[test]
    fn parses_all_current_administrative_levels_and_populated_places() {
        let country = bundled_countries()
            .expect("countries should parse")
            .into_iter()
            .find(|country| country.code == "VN")
            .expect("Vietnam should exist");
        let admin5 =
            parse_admin5_codes(Cursor::new("500\t05\n600\t05\n")).expect("ADM5 codes should parse");

        let parsed = parse_country_dump(&country, Cursor::new(DUMP), &admin5)
            .expect("country dump should parse");

        assert_eq!(parsed.areas.len(), 6);
        assert_eq!(parsed.source_updated_at.as_deref(), Some("2026-08-06"));
        assert_eq!(parsed.bbox, Some([10.0, 15.0, 20.0, 25.0]));
        let city = parsed
            .areas
            .iter()
            .find(|area| area.geoname_id == 600)
            .expect("city should exist");
        assert_eq!(city.parent_id, Some(500));
        assert_eq!(city.kind, AdministrativeAreaKind::PopulatedPlace);
        assert_eq!(city.administrative_level, None);
        assert_eq!(city.alternate_names, "City Zeta,Zeta");
        assert_eq!(
            city.canonical_path,
            "Zeta City, Epsilon Hamlet, Delta Ward, Gamma Commune, Beta District, Alpha Province, Vietnam"
        );
    }

    #[test]
    fn rejects_malformed_supported_rows_with_the_source_line_number() {
        let country = bundled_countries()
            .expect("countries should parse")
            .into_iter()
            .find(|country| country.code == "VN")
            .expect("Vietnam should exist");

        let error = parse_country_dump(
            &country,
            Cursor::new("100\tBroken\tBroken\n"),
            &Default::default(),
        )
        .expect_err("malformed rows must fail");

        assert!(error.to_string().contains("row 1"));
    }

    #[test]
    fn admin5_codes_reject_duplicate_geoname_ids() {
        let error = parse_admin5_codes(Cursor::new("500\t05\n500\t06\n"))
            .expect_err("duplicate ADM5 mappings must fail");

        assert!(error.to_string().contains("duplicate"));
    }
}
