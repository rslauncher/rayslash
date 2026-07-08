#[derive(Debug, Clone, PartialEq)]
pub struct UnitConversion {
    pub expression: String,
    pub result: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dimension {
    Length,
    Mass,
    Volume,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TemperatureScale {
    Celsius,
    Fahrenheit,
    Kelvin,
}

#[derive(Debug, Clone, Copy)]
enum UnitKind {
    Linear {
        dimension: Dimension,
        factor: f64,
        symbol: &'static str,
    },
    Temperature {
        scale: TemperatureScale,
        symbol: &'static str,
    },
}

pub fn convert_query(query: &str) -> Option<UnitConversion> {
    let query = query.trim();
    let (left, target_unit) = split_conversion_query(query)?;
    let (amount, source_unit) = parse_amount_and_unit(left)?;
    let source = unit_for(source_unit)?;
    let target = unit_for(target_unit)?;
    let converted = convert(amount, source, target)?;

    Some(UnitConversion {
        expression: format!(
            "{} {} to {}",
            format_number(amount, 4),
            source.symbol(),
            target.symbol()
        ),
        result: format!(
            "{} {}",
            format_number(converted, target.result_decimals()),
            target.symbol()
        ),
    })
}

fn split_conversion_query(query: &str) -> Option<(&str, &str)> {
    split_once_word(query, "to").or_else(|| split_once_word(query, "in"))
}

fn split_once_word<'a>(query: &'a str, word: &str) -> Option<(&'a str, &'a str)> {
    let lower = query.to_ascii_lowercase();
    let needle = format!(" {word} ");
    let index = lower.rfind(&needle)?;
    let left = query[..index].trim();
    let right = query[index + needle.len()..].trim();

    (!left.is_empty() && !right.is_empty()).then_some((left, right))
}

fn parse_amount_and_unit(text: &str) -> Option<(f64, &str)> {
    let text = text.trim();
    let mut end = 0;
    let mut seen_digit = false;

    for (index, ch) in text.char_indices() {
        let valid = ch.is_ascii_digit()
            || ch == '.'
            || ((ch == '+' || ch == '-') && index == 0)
            || ((ch == 'e' || ch == 'E') && seen_digit);
        if !valid {
            break;
        }
        if ch.is_ascii_digit() {
            seen_digit = true;
        }
        end = index + ch.len_utf8();
    }

    if !seen_digit {
        return None;
    }

    let amount = text[..end].parse::<f64>().ok()?;
    let unit = text[end..].trim();
    (!unit.is_empty()).then_some((amount, unit))
}

fn convert(amount: f64, source: UnitKind, target: UnitKind) -> Option<f64> {
    match (source, target) {
        (
            UnitKind::Linear {
                dimension: source_dimension,
                factor: source_factor,
                ..
            },
            UnitKind::Linear {
                dimension: target_dimension,
                factor: target_factor,
                ..
            },
        ) if source_dimension == target_dimension => Some(amount * source_factor / target_factor),
        (
            UnitKind::Temperature {
                scale: source_scale,
                ..
            },
            UnitKind::Temperature {
                scale: target_scale,
                ..
            },
        ) => Some(from_celsius(to_celsius(amount, source_scale), target_scale)),
        _ => None,
    }
    .filter(|value| value.is_finite())
}

impl UnitKind {
    fn symbol(self) -> &'static str {
        match self {
            UnitKind::Linear { symbol, .. } | UnitKind::Temperature { symbol, .. } => symbol,
        }
    }

    fn result_decimals(self) -> usize {
        match self {
            UnitKind::Linear { .. } => 4,
            UnitKind::Temperature { .. } => 2,
        }
    }
}

fn unit_for(unit: &str) -> Option<UnitKind> {
    let unit = normalize_unit_name(unit);

    match unit.as_str() {
        "m" | "meter" | "meters" | "metre" | "metres" => linear(Dimension::Length, 1.0, "m"),
        "km" | "kilometer" | "kilometers" | "kilometre" | "kilometres" => {
            linear(Dimension::Length, 1000.0, "km")
        }
        "cm" | "centimeter" | "centimeters" | "centimetre" | "centimetres" => {
            linear(Dimension::Length, 0.01, "cm")
        }
        "mm" | "millimeter" | "millimeters" | "millimetre" | "millimetres" => {
            linear(Dimension::Length, 0.001, "mm")
        }
        "mi" | "mile" | "miles" => linear(Dimension::Length, 1609.344, "mi"),
        "yd" | "yard" | "yards" => linear(Dimension::Length, 0.9144, "yd"),
        "ft" | "foot" | "feet" => linear(Dimension::Length, 0.3048, "ft"),
        "in" | "inch" | "inches" => linear(Dimension::Length, 0.0254, "in"),

        "kg" | "kilogram" | "kilograms" => linear(Dimension::Mass, 1.0, "kg"),
        "g" | "gram" | "grams" => linear(Dimension::Mass, 0.001, "g"),
        "mg" | "milligram" | "milligrams" => linear(Dimension::Mass, 0.000001, "mg"),
        "lb" | "lbs" | "pound" | "pounds" => linear(Dimension::Mass, 0.45359237, "lb"),
        "oz" | "ounce" | "ounces" => linear(Dimension::Mass, 0.028349523125, "oz"),

        "l" | "liter" | "liters" | "litre" | "litres" => linear(Dimension::Volume, 1.0, "L"),
        "ml" | "milliliter" | "milliliters" | "millilitre" | "millilitres" => {
            linear(Dimension::Volume, 0.001, "mL")
        }
        "gal" | "gallon" | "gallons" => linear(Dimension::Volume, 3.785411784, "gal"),
        "qt" | "quart" | "quarts" => linear(Dimension::Volume, 0.946352946, "qt"),
        "pt" | "pint" | "pints" => linear(Dimension::Volume, 0.473176473, "pt"),
        "cup" | "cups" => linear(Dimension::Volume, 0.2365882365, "cup"),
        "floz" | "fl oz" | "fluid ounce" | "fluid ounces" => {
            linear(Dimension::Volume, 0.0295735295625, "fl oz")
        }
        "tbsp" | "tablespoon" | "tablespoons" => {
            linear(Dimension::Volume, 0.01478676478125, "tbsp")
        }
        "tsp" | "teaspoon" | "teaspoons" => linear(Dimension::Volume, 0.00492892159375, "tsp"),

        "c" | "celsius" | "degree celsius" | "degrees celsius" => {
            temperature(TemperatureScale::Celsius, "°C")
        }
        "f" | "fahrenheit" | "degree fahrenheit" | "degrees fahrenheit" => {
            temperature(TemperatureScale::Fahrenheit, "°F")
        }
        "k" | "kelvin" => temperature(TemperatureScale::Kelvin, "K"),
        _ => None,
    }
}

fn linear(dimension: Dimension, factor: f64, symbol: &'static str) -> Option<UnitKind> {
    Some(UnitKind::Linear {
        dimension,
        factor,
        symbol,
    })
}

fn temperature(scale: TemperatureScale, symbol: &'static str) -> Option<UnitKind> {
    Some(UnitKind::Temperature { scale, symbol })
}

fn normalize_unit_name(unit: &str) -> String {
    unit.trim()
        .to_ascii_lowercase()
        .replace('°', "")
        .replace(['_', '-'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn to_celsius(value: f64, scale: TemperatureScale) -> f64 {
    match scale {
        TemperatureScale::Celsius => value,
        TemperatureScale::Fahrenheit => (value - 32.0) * 5.0 / 9.0,
        TemperatureScale::Kelvin => value - 273.15,
    }
}

fn from_celsius(value: f64, scale: TemperatureScale) -> f64 {
    match scale {
        TemperatureScale::Celsius => value,
        TemperatureScale::Fahrenheit => value * 9.0 / 5.0 + 32.0,
        TemperatureScale::Kelvin => value + 273.15,
    }
}

fn format_number(value: f64, max_decimals: usize) -> String {
    if (value - value.round()).abs() < 0.000000001 {
        return format!("{:.0}", value.round());
    }

    let mut text = format!("{value:.max_decimals$}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }

    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_common_length_mass_volume_and_temperature_units() {
        assert_eq!(
            convert_query("10 km to mi").expect("length").result,
            "6.2137 mi"
        );
        assert_eq!(
            convert_query("10 miles to km").expect("length").result,
            "16.0934 km"
        );
        assert_eq!(
            convert_query("2 lb to kg").expect("mass").result,
            "0.9072 kg"
        );
        assert_eq!(
            convert_query("1 cup to ml").expect("volume").result,
            "236.5882 mL"
        );
        assert_eq!(
            convert_query("32 f to c").expect("temperature").result,
            "0 °C"
        );
        assert_eq!(
            convert_query("10 f to c").expect("temperature").result,
            "-12.22 °C"
        );
        assert_eq!(
            convert_query("10c to k")
                .expect("temperature shorthand")
                .result,
            "283.15 K"
        );
        assert_eq!(
            convert_query("10 celsius to fahrenheit")
                .expect("temperature names")
                .expression,
            "10 °C to °F"
        );
    }

    #[test]
    fn conversion_requires_supported_units_in_same_dimension() {
        assert!(convert_query("10 km to kg").is_none());
        assert!(convert_query("10 widgets to m").is_none());
        assert!(convert_query("code to m").is_none());
    }

    #[test]
    fn conversion_accepts_in_as_connector() {
        assert_eq!(
            convert_query("12 in in cm").expect("inches").result,
            "30.48 cm"
        );
    }
}
