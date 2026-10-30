use trading_agent_kis_client::domain::BrokerEnvironment;
use trading_agent_kis_client::transport::{EndpointGroup, KisEndpoint, KisHttpMethod};

pub const OFFICIAL_OPEN_TRADING_API_COMMIT: &str = "33e0e1e65cd1c8c8b639531483ec0b327087bab1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashkeyPolicy {
    DisabledByOfficialSample,
    Required,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApiSpecCase {
    pub official_source_path: &'static str,
    pub checked_commit: &'static str,
    pub endpoint_name: &'static str,
    pub method: KisHttpMethod,
    pub path: &'static str,
    pub live_tr_id: &'static str,
    pub paper_tr_id: &'static str,
    pub group: EndpointGroup,
    pub hashkey_policy: HashkeyPolicy,
    pub required_headers: &'static [&'static str],
    pub required_query_fields: &'static [&'static str],
    pub required_body_fields: &'static [&'static str],
    pub pagination_token_fields: &'static [&'static str],
    pub success_fixture: Option<&'static str>,
    pub error_fixture: Option<&'static str>,
}

impl ApiSpecCase {
    pub fn assert_endpoint(self, paper: KisEndpoint, live: KisEndpoint) {
        assert_eq!(self.checked_commit, OFFICIAL_OPEN_TRADING_API_COMMIT);
        assert!(!self.official_source_path.trim().is_empty());
        assert_eq!(paper.name, self.endpoint_name);
        assert_eq!(live.name, self.endpoint_name);
        assert_eq!(paper.method, self.method);
        assert_eq!(live.method, self.method);
        assert_eq!(paper.path, self.path);
        assert_eq!(live.path, self.path);
        assert_eq!(paper.tr_id, self.paper_tr_id);
        assert_eq!(live.tr_id, self.live_tr_id);
        assert_eq!(paper.group, self.group);
        assert_eq!(live.group, self.group);
        assert_eq!(
            paper.requires_hashkey,
            self.hashkey_policy == HashkeyPolicy::Required
        );
        assert_eq!(
            live.requires_hashkey,
            self.hashkey_policy == HashkeyPolicy::Required
        );
    }

    pub fn assert_lists_are_defined(self) {
        assert!(
            self.required_headers
                .iter()
                .all(|field| !field.trim().is_empty())
        );
        assert!(
            self.required_query_fields
                .iter()
                .all(|field| !field.trim().is_empty())
        );
        assert!(
            self.required_body_fields
                .iter()
                .all(|field| !field.trim().is_empty())
        );
        assert!(
            self.pagination_token_fields
                .iter()
                .all(|field| !field.trim().is_empty())
        );
    }
}

pub const COMMON_REST_HEADERS: &[&str] = &[
    "content-type",
    "authorization",
    "appkey",
    "appsecret",
    "tr_id",
    "custtype",
    "tr_cont",
];

pub fn paper_tr_id_from_official_wrapper(live_tr_id: &'static str) -> String {
    let Some(suffix) = live_tr_id.strip_prefix(['T', 'J', 'C']) else {
        return live_tr_id.to_owned();
    };
    format!("V{suffix}")
}

pub fn assert_environment(endpoint: KisEndpoint, environment: BrokerEnvironment) {
    match environment {
        BrokerEnvironment::Paper => assert!(endpoint.tr_id.starts_with('V')),
        BrokerEnvironment::Live => assert!(!endpoint.tr_id.starts_with('V')),
        BrokerEnvironment::Mock => panic!("mock should not have KIS endpoint contracts"),
    }
}
