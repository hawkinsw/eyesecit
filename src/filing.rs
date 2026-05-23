use chrono::{DateTime, Duration, FixedOffset};
use serde::de::Error;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filing {
    pub items: String,
    pub form: String,
    pub acceptance_date_times: DateTime<FixedOffset>,
}

impl Filing {
    fn new(items: String, form: String, time: DateTime<FixedOffset>) -> Self {
        Filing {
            items,
            form,
            acceptance_date_times: time,
        }
    }
}

pub struct Filings {
    pub filings: Vec<Filing>,
}

impl TryFrom<Value> for Filings {
    type Error = serde_json::Error;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let filings = value
            .as_object()
            .and_then(|obj| obj.get("filings"))
            .ok_or(serde_json::Error::custom("Could not find filings."))?;
        let recent = filings
            .as_object()
            .and_then(|obj| obj.get("recent"))
            .ok_or(serde_json::Error::custom("Could not find recent"))?;
        let forms = recent
            .as_object()
            .and_then(|obj| obj.get("form"))
            .and_then(|obj| obj.as_array())
            .ok_or(serde_json::Error::custom("Could not find forms."))?;
        let items = recent
            .as_object()
            .and_then(|obj| obj.get("items"))
            .and_then(|obj| obj.as_array())
            .ok_or(serde_json::Error::custom("Could not find items."))?;
        let acceptance_date_times: Vec<DateTime<FixedOffset>> = recent
            .as_object()
            .and_then(|obj| obj.get("acceptanceDateTime"))
            .and_then(|obj| obj.as_array())
            .map(|obj| {
                obj.iter()
                    .map_while(|raw| {
                        raw.as_str()
                            .and_then(|raw_str| DateTime::parse_from_rfc3339(raw_str).ok())
                    })
                    .collect()
            })
            .map(|obj: Vec<DateTime<FixedOffset>>| {
                obj.into_iter()
                    .map(|time| time + Duration::hours(4))
                    .collect()
            })
            .ok_or(serde_json::Error::custom(
                "Could not find acceptance date/times.",
            ))?;
        if forms.len() != items.len() || items.len() != acceptance_date_times.len() {
            return Err(serde_json::Error::custom(
                "Corrupt filings JSON: Length of arrays do not match",
            ));
        }

        let mut result: Vec<Filing> = Vec::new();

        for index in 0..forms.len() {
            result.push(Filing::new(
                items[index].to_string(),
                forms[index].to_string(),
                acceptance_date_times[index],
            ));
        }
        Ok(Filings { filings: result })
    }
}

#[cfg(test)]
mod test {

    use chrono::NaiveDate;
    use serde_json::Value;

    use crate::filing::{Filing, Filings};
    #[test]
    fn test_parse_recent_form_slim() {
        let file = std::fs::File::open("test_data/slim.json").unwrap();
        let raw_value: Result<Value, serde_json::Error> = serde_json::from_reader(file);
        assert!(raw_value.is_ok());
        let extract_result = TryInto::<Filings>::try_into(raw_value.unwrap());
        assert!(extract_result.is_ok());
        assert!(extract_result.unwrap().filings.len() == 3);
    }

    #[test]
    fn test_parse_recent_form_uneven() {
        let file = std::fs::File::open("test_data/uneven.json").unwrap();
        let raw_value: Result<Value, serde_json::Error> = serde_json::from_reader(file);
        assert!(raw_value.is_ok());
        let extract_result = TryInto::<Filings>::try_into(raw_value.unwrap());
        assert!(extract_result
            .is_err_and(|err| err.to_string().contains("Length of arrays do not match")));
    }

    #[test]
    fn test_parse_recent_form_filter_by_date() {
        let now = NaiveDate::from_ymd_opt(2024, 5, 1)
            .expect("")
            .and_hms_opt(0, 0, 0)
            .expect("")
            .and_utc();
        let file = std::fs::File::open("test_data/slim.json").unwrap();
        let raw_value: Value =
            serde_json::from_reader(file).expect("Should be able to parse test data file.");

        let extract_result = TryInto::<Filings>::try_into(raw_value)
            .expect("Should be able to extract filings metadata.");
        let filtered_result = extract_result
            .filings
            .into_iter()
            .filter(|filing| filing.acceptance_date_times > now);
        assert!(filtered_result.collect::<Vec<Filing>>().len() == 1);

        assert!(true);
    }
}
