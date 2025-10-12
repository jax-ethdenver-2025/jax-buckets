use std::str::FromStr;

use mime::Mime;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq)]
pub struct MaybeMime(pub Option<Mime>);

impl Serialize for MaybeMime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match &self.0 {
            Some(mime) => serializer.serialize_str(mime.as_ref()),
            None => serializer.serialize_none(),
        }
    }
}

impl<'de> Deserialize<'de> for MaybeMime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<String> = Option::deserialize(deserializer)?;
        match opt {
            Some(s) => {
                let mime = Mime::from_str(&s).map_err(serde::de::Error::custom)?;
                Ok(MaybeMime(Some(mime)))
            }
            None => Ok(MaybeMime(None)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_some_mime() {
        let mime = "text/plain".parse::<Mime>().unwrap();
        let maybe_mime = MaybeMime(Some(mime));
        let json = serde_json::to_string(&maybe_mime).unwrap();
        assert_eq!(json, r#""text/plain""#);
    }

    #[test]
    fn test_serialize_none_mime() {
        let maybe_mime = MaybeMime(None);
        let json = serde_json::to_string(&maybe_mime).unwrap();
        assert_eq!(json, "null");
    }

    #[test]
    fn test_deserialize_some_mime() {
        let json = r#""text/html; charset=utf-8""#;
        let maybe_mime: MaybeMime = serde_json::from_str(json).unwrap();
        let expected_mime = "text/html; charset=utf-8".parse::<Mime>().unwrap();
        assert_eq!(maybe_mime, MaybeMime(Some(expected_mime)));
    }

    #[test]
    fn test_deserialize_none_mime() {
        let json = "null";
        let maybe_mime: MaybeMime = serde_json::from_str(json).unwrap();
        assert_eq!(maybe_mime, MaybeMime(None));
    }

    #[test]
    fn test_roundtrip_some() {
        let mime = "application/json".parse::<Mime>().unwrap();
        let original = MaybeMime(Some(mime.clone()));
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: MaybeMime = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
        assert_eq!(deserialized.0, Some(mime));
    }

    #[test]
    fn test_roundtrip_none() {
        let original = MaybeMime(None);
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: MaybeMime = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
        assert_eq!(deserialized.0, None);
    }

    #[test]
    fn test_deserialize_invalid_mime() {
        let json = r#""invalid/mime/type/with/too/many/parts""#;
        let result: Result<MaybeMime, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_complex_mime_types() {
        let test_cases = vec![
            "text/plain",
            "application/json",
            "image/png",
            "text/html; charset=utf-8",
            "multipart/form-data; boundary=something",
            "application/vnd.api+json",
        ];

        for mime_str in test_cases {
            let mime = mime_str.parse::<Mime>().unwrap();
            let maybe_mime = MaybeMime(Some(mime.clone()));
            let json = serde_json::to_string(&maybe_mime).unwrap();
            let deserialized: MaybeMime = serde_json::from_str(&json).unwrap();
            assert_eq!(maybe_mime, deserialized);
            assert_eq!(deserialized.0, Some(mime));
        }
    }

    #[test]
    fn test_debug_formatting() {
        let maybe_mime_some = MaybeMime(Some("text/plain".parse().unwrap()));
        let maybe_mime_none = MaybeMime(None);

        assert_eq!(
            format!("{:?}", maybe_mime_some),
            "MaybeMime(Some(\"text/plain\"))"
        );
        assert_eq!(format!("{:?}", maybe_mime_none), "MaybeMime(None)");
    }

    #[test]
    fn test_clone_and_equality() {
        let mime = "application/xml".parse::<Mime>().unwrap();
        let maybe_mime1 = MaybeMime(Some(mime.clone()));
        let maybe_mime2 = maybe_mime1.clone();

        assert_eq!(maybe_mime1, maybe_mime2);
        assert_eq!(maybe_mime1.0, Some(mime));
    }
}
