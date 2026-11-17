use crate::model::{self, Error, Field};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub enum Operator {
    #[serde(rename = "=")]
    Equal,
    #[serde(rename = "<>")]
    NotEqual,
    #[serde(rename = "<")]
    Less,
    #[serde(rename = "<=")]
    LessEqual,
    #[serde(rename = ">")]
    Greater,
    #[serde(rename = ">=")]
    GreaterEqual,
    #[serde(rename = "LIKE")]
    Like,
    #[serde(rename = "ILIKE")]
    ILike,
    #[serde(rename = "IS NULL")]
    IsNull,
    #[serde(rename = "IS NOT NULL")]
    IsNotNull,
}

#[derive(Debug, Deserialize)]
pub enum Join {
    Replace,
    #[serde(rename = "AND")]
    And,
    #[serde(rename = "OR")]
    Or,
}

#[derive(Debug, Deserialize)]
pub struct Condition {
    pub operator: Operator,
    pub value: String,
    pub combine: Join,
}

impl Condition {
    pub fn expression(&self, field: &Field, existing: &str) -> Result<String, Error> {
        let name = format!("\"{}\"", field.name.replace('"', "\"\""));
        let op = match self.operator {
            Operator::Equal => "=",
            Operator::NotEqual => "<>",
            Operator::Less => "<",
            Operator::LessEqual => "<=",
            Operator::Greater => ">",
            Operator::GreaterEqual => ">=",
            Operator::Like => "LIKE",
            Operator::ILike => "ILIKE",
            Operator::IsNull => "IS NULL",
            Operator::IsNotNull => "IS NOT NULL",
        };
        let clause = match self.operator {
            Operator::IsNull | Operator::IsNotNull => format!("{name} {op}"),
            Operator::Equal
            | Operator::NotEqual
            | Operator::Less
            | Operator::LessEqual
            | Operator::Greater
            | Operator::GreaterEqual
            | Operator::Like
            | Operator::ILike => {
                if matches!(self.operator, Operator::Like | Operator::ILike) && field.kind != "Text"
                {
                    return Err(Error::Invalid("LIKE and ILIKE require a text field".into()));
                }
                let value = match model::literal(field, &self.value, false)? {
                    Value::String(text) => format!("'{}'", text.replace('\'', "''")),
                    Value::Number(number) => number.to_string(),
                    Value::Bool(true) => "TRUE".into(),
                    Value::Bool(false) => "FALSE".into(),
                    Value::Null | Value::Array(_) | Value::Object(_) => {
                        return Err(Error::Invalid(
                            "Choose IS NULL or a supported scalar value".into(),
                        ));
                    }
                };
                format!("{name} {op} {value}")
            }
        };
        let query = match (&self.combine, existing.trim()) {
            (Join::Replace, _) | (Join::And | Join::Or, "") => clause,
            (Join::And, old) => format!("({old}) AND ({clause})"),
            (Join::Or, old) => format!("({old}) OR ({clause})"),
        };
        if query.len() > 4000 {
            return Err(Error::Invalid("Expression exceeds 4000 UTF-8 bytes".into()));
        }
        Ok(query)
    }
}

#[cfg(test)]
#[path = "sql_builder_tests.rs"]
mod tests;
