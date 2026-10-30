#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EndpointGroup {
    Auth,
    ReadLight,
    ReadHeavy,
    WriteOrder,
}

impl EndpointGroup {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auth => "auth",
            Self::ReadLight => "read_light",
            Self::ReadHeavy => "read_heavy",
            Self::WriteOrder => "write_order",
        }
    }

    pub fn should_retry(self, failure: TransportFailureClass) -> bool {
        match (self, failure) {
            (_, TransportFailureClass::Validation | TransportFailureClass::AmbiguousAfterWrite) => {
                false
            }
            (Self::WriteOrder, _) => false,
            (Self::ReadLight | Self::ReadHeavy, TransportFailureClass::RateLimit) => true,
            (_, TransportFailureClass::RateLimit) => false,
            (Self::Auth, TransportFailureClass::Connect | TransportFailureClass::ReadTimeout) => {
                true
            }
            (Self::ReadLight | Self::ReadHeavy, _) => true,
            (Self::Auth, TransportFailureClass::Http5xx) => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportFailureClass {
    Connect,
    ReadTimeout,
    Http5xx,
    RateLimit,
    Validation,
    AmbiguousAfterWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KisHttpMethod {
    Get,
    Post,
}

impl KisHttpMethod {
    pub(super) fn as_reqwest_method(self) -> reqwest::Method {
        match self {
            Self::Get => reqwest::Method::GET,
            Self::Post => reqwest::Method::POST,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KisEndpoint {
    pub name: &'static str,
    pub method: KisHttpMethod,
    pub path: &'static str,
    pub tr_id: &'static str,
    pub group: EndpointGroup,
    pub requires_hashkey: bool,
}

pub(super) const HASHKEY_ENDPOINT: KisEndpoint = KisEndpoint {
    name: "hashkey",
    method: KisHttpMethod::Post,
    path: "/uapi/hashkey",
    tr_id: "",
    group: EndpointGroup::Auth,
    requires_hashkey: false,
};
