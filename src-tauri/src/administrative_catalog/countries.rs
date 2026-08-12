use std::collections::HashSet;

use anyhow::{anyhow, Context, Result};

use super::types::AdministrativeCountry;

pub fn bundled_countries() -> Result<Vec<AdministrativeCountry>> {
    parse_countries(include_str!("../../resources/geonames-country-info.txt"))
}

fn parse_countries(input: &str) -> Result<Vec<AdministrativeCountry>> {
    let mut countries = Vec::new();
    let mut seen_codes = HashSet::new();

    for (index, line) in input.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 18 {
            return Err(anyhow!(
                "GeoNames country row {} has {} columns, expected at least 18",
                index + 1,
                fields.len()
            ));
        }

        let code = fields[0].trim().to_ascii_uppercase();
        let iso3 = fields[1].trim().to_ascii_uppercase();
        if code.len() != 2 || !code.bytes().all(|byte| byte.is_ascii_alphabetic()) {
            return Err(anyhow!("invalid ISO alpha-2 code on row {}", index + 1));
        }
        if iso3.len() != 3 || !iso3.bytes().all(|byte| byte.is_ascii_alphabetic()) {
            return Err(anyhow!("invalid ISO alpha-3 code on row {}", index + 1));
        }
        if !seen_codes.insert(code.clone()) {
            return Err(anyhow!("duplicate country code {code}"));
        }

        let geoname_id = fields[16]
            .trim()
            .parse::<i64>()
            .with_context(|| format!("invalid GeoNames country ID on row {}", index + 1))?;
        countries.push(AdministrativeCountry {
            code,
            iso3,
            name: fields[4].trim().to_string(),
            capital: fields[5].trim().to_string(),
            continent_code: fields[8].trim().to_string(),
            languages: fields[15]
                .split(',')
                .map(str::trim)
                .filter(|language| !language.is_empty())
                .map(str::to_string)
                .collect(),
            geoname_id,
        });
    }

    countries.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.code.cmp(&right.code))
    });
    Ok(countries)
}

#[cfg(test)]
mod tests {
    use super::bundled_countries;

    #[test]
    fn bundled_catalog_contains_the_complete_iso_country_set() {
        let countries = bundled_countries().expect("country snapshot should parse");

        assert!(countries.len() >= 249, "got {} countries", countries.len());
        for code in ["VN", "KR", "US", "AQ"] {
            assert!(
                countries.iter().any(|country| country.code == code),
                "missing {code}"
            );
        }
        assert!(countries
            .windows(2)
            .all(|pair| pair[0].name <= pair[1].name));
    }

    #[test]
    fn bundled_catalog_preserves_country_metadata() {
        let countries = bundled_countries().expect("country snapshot should parse");
        let vietnam = countries
            .iter()
            .find(|country| country.code == "VN")
            .expect("Vietnam should exist");

        assert_eq!(vietnam.iso3, "VNM");
        assert_eq!(vietnam.continent_code, "AS");
        assert_eq!(vietnam.capital, "Hanoi");
        assert!(vietnam.languages.iter().any(|language| language == "vi"));
        assert_eq!(vietnam.geoname_id, 1_562_822);
    }
}
