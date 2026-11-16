//! Per-image quotes are selected before credit is reserved or an upstream is called.
use super::{BillingMeter, ImagePriceTier, Price};
use crate::error::{AppError, AppResult};
use std::collections::HashSet;

pub fn normalize_spec(value: &str) -> String {
    let value = value
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '_'], "")
        .replace('×', "x");
    match value.as_str() {
        "1024" | "1024p" | "1k" => "1k".into(),
        "2048" | "2048p" | "2k" => "2k".into(),
        "4096" | "4096p" | "4k" => "4k".into(),
        "" => "auto".into(),
        _ => value,
    }
}
fn size_bucket(value: &str) -> Option<String> {
    let (w, h) = value.split_once('x')?;
    let edge = w.parse::<u32>().ok()?.max(h.parse::<u32>().ok()?);
    match edge {
        1024 => Some("1k".into()),
        2048 => Some("2k".into()),
        4096 => Some("4k".into()),
        _ => None,
    }
}
pub fn validate_tiers(meter: BillingMeter, tiers: &[ImagePriceTier]) -> AppResult<()> {
    if tiers.is_empty() {
        return Ok(());
    }
    if meter != BillingMeter::Image || tiers.len() > 100 {
        return Err(AppError::BadRequest(
            "图像价格档位仅支持按张计费，最多 100 档".into(),
        ));
    }
    let mut seen = HashSet::new();
    for tier in tiers {
        if tier.size.trim().is_empty()
            || tier.quality.trim().is_empty()
            || tier.size.len() > 64
            || tier.quality.len() > 64
            || tier.unit_price_micros <= 0
            || !seen.insert((normalize_spec(&tier.size), normalize_spec(&tier.quality)))
        {
            return Err(AppError::BadRequest(
                "图像规格不能为空或重复，且每档单价必须大于零".into(),
            ));
        }
    }
    Ok(())
}
pub fn apply_tier(price: &mut Price, size: Option<&str>, quality: Option<&str>) -> AppResult<()> {
    if price.billing_meter != BillingMeter::Image || price.image_price_tiers.is_empty() {
        return Ok(());
    }
    let size = normalize_spec(size.unwrap_or("auto"));
    let quality = normalize_spec(quality.unwrap_or("auto"));
    let bucket = size_bucket(&size);
    let selected = price
        .image_price_tiers
        .iter()
        .filter_map(|tier| {
            let candidate = normalize_spec(&tier.size);
            let size_score = if candidate == size {
                4
            } else if bucket.as_ref() == Some(&candidate) {
                2
            } else if candidate == "*" {
                0
            } else {
                return None;
            };
            let candidate_quality = normalize_spec(&tier.quality);
            let quality_score = if candidate_quality == quality {
                1
            } else if candidate_quality == "*" {
                0
            } else {
                return None;
            };
            Some((size_score + quality_score, tier.unit_price_micros))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, price)| price)
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "图像价格未配置：size={size}, quality={quality}；请使用模型广场列出的规格"
            ))
        })?;
    price.unit_price_micros = Some(selected);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn price() -> Price {
        Price {
            input_price_micros: 0,
            output_price_micros: 0,
            cache_read_price_micros: None,
            cache_write_price_micros: None,
            billing_meter: BillingMeter::Image,
            unit_price_micros: Some(10),
            video_billing_mode: None,
            video_price_tiers: vec![],
            image_price_tiers: vec![
                ImagePriceTier {
                    size: "*".into(),
                    quality: "*".into(),
                    unit_price_micros: 100,
                },
                ImagePriceTier {
                    size: "2k".into(),
                    quality: "*".into(),
                    unit_price_micros: 200,
                },
                ImagePriceTier {
                    size: "2k".into(),
                    quality: "high".into(),
                    unit_price_micros: 300,
                },
            ],
        }
    }
    #[test]
    fn selects_dimensions_quality_and_specific_tier_before_wildcard() {
        for (size, quality, expected) in [
            ("2048x2048", "high", 300),
            ("2K", "standard", 200),
            ("1K", "auto", 100),
        ] {
            let mut p = price();
            apply_tier(&mut p, Some(size), Some(quality)).unwrap();
            assert_eq!(p.unit_price_micros, Some(expected));
            assert_eq!(
                super::super::cost_for_billable_usage(super::super::BillableUsage::image(3), &p),
                expected * 3
            );
        }
    }
    #[test]
    fn rejects_unpriced_specs_without_falling_back_to_legacy_price() {
        let mut p = price();
        p.image_price_tiers.remove(0);
        assert!(apply_tier(&mut p, Some("4k"), None).is_err());
        assert!(apply_tier(&mut p, None, None).is_err());
        p.image_price_tiers.clear();
        apply_tier(&mut p, None, None).unwrap();
        assert_eq!(p.unit_price_micros, Some(10));
    }
    #[test]
    fn rejects_duplicates_invalid_prices_and_wrong_meter() {
        let mut tiers = price().image_price_tiers;
        assert!(validate_tiers(BillingMeter::Image, &tiers).is_ok());
        assert!(validate_tiers(BillingMeter::Token, &tiers).is_err());
        tiers.push(ImagePriceTier {
            size: "2048p".into(),
            quality: "high".into(),
            unit_price_micros: 300,
        });
        assert!(validate_tiers(BillingMeter::Image, &tiers).is_err());
        tiers.pop();
        tiers[0].unit_price_micros = 0;
        assert!(validate_tiers(BillingMeter::Image, &tiers).is_err());
    }
}
