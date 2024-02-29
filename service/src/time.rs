use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use rocket::form::{
    error::{ErrorKind, Errors},
    FromFormField, ValueField,
};

pub struct NaiveDateParam(NaiveDate);

impl From<&NaiveDateParam> for DateTime<Utc> {
    fn from(value: &NaiveDateParam) -> Self {
        value.0.and_time(NaiveTime::MIN).and_utc()
    }
}

#[rocket::async_trait]
impl<'v> FromFormField<'v> for NaiveDateParam {
    fn from_value(field: ValueField<'v>) -> rocket::form::Result<'v, Self> {
        NaiveDate::parse_from_str(field.value, "%Y-%m-%d")
            .map(NaiveDateParam)
            .map_err(|_| {
                let mut errors = Errors::from(ErrorKind::Validation("NaiveDate".into()));
                errors.set_name(field.name);
                errors.set_value(field.value);
                errors
            })
    }
}
