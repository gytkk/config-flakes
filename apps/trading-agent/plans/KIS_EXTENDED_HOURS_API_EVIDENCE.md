# KIS Extended Hours API Evidence

Access date: 2026-05-01

## Premarket and Aftermarket

Official source:

- KIS Developers API guide: https://apiportal.koreainvestment.com/apiservice-apiservice
- Public API metadata endpoint used by the guide: https://apiportal.koreainvestment.com/api/apis/public/api-list/fae9f1bb-0de9-4523-ab91-6c534e212700
- Official sample: https://github.com/koreainvestment/open-trading-api/blob/main/examples_llm/overseas_stock/order/order.py

Confirmed API:

| Item | Value |
| --- | --- |
| API name | 해외주식 주문[v1_해외주식-001] |
| Method | POST |
| Path | `/uapi/overseas-stock/v1/trading/order` |
| Live domain | `https://openapi.koreainvestment.com:9443` |
| Paper domain | `https://openapivts.koreainvestment.com:29443` |
| US buy TR ID | `TTTT1002U` |
| US sell TR ID | `TTTT1006U` |
| Paper US buy TR ID | `VTTT1002U` |
| Paper US sell TR ID | `VTTT1001U` |

Official guide evidence:

- The API summary states that US premarket and aftermarket time windows are orderable through the overseas stock order API.
- Listed US regular session: Korea time `23:30-06:00`, summer time `22:30-05:00`.
- Listed US premarket: Korea time `18:00-23:30`, summer time `17:00-22:30`.
- Listed US aftermarket: Korea time `06:00-07:00`, summer time `05:00-07:00`.
- The same summary says US daytime trading should use the separate `해외주식 미국주간주문` API.

Official sample evidence:

- `order.py` defines `API_URL = "/uapi/overseas-stock/v1/trading/order"`.
- For US buy orders, the sample selects `TTTT1002U`.
- For US sell orders, the sample selects `TTTT1006U`.
- The official sample comments and demo-mode transformation imply paper US sell
  `VTTT1006U`, but the KIS Developers API metadata lists paper US sell
  `VTTT1001U`. Implementation should follow the API metadata contract and keep
  this conflict visible in endpoint contract tests.
- The sample documents US `ORD_DVSN` values including `00` limit, `31` MOO, `32` LOO, `33` MOC, and `34` LOC depending on side.

## US Daytime Trading

Official source:

- KIS Developers API guide: https://apiportal.koreainvestment.com/apiservice-apiservice
- Public API metadata endpoint used by the guide: https://apiportal.koreainvestment.com/api/apis/public/api-list/fae9f1bb-0de9-4523-ab91-6c534e212700
- Official sample: https://github.com/koreainvestment/open-trading-api/blob/main/examples_llm/overseas_stock/daytime_order/daytime_order.py
- Korea Investment foreign securities guide: https://file.koreainvestment.com/Storage/customer/guide/regards/service_03_1%283%29.htm

Confirmed API:

| Item | Value |
| --- | --- |
| API name | 해외주식 미국주간주문[v1_해외주식-026] |
| Method | POST |
| Path | `/uapi/overseas-stock/v1/trading/daytime-order` |
| Live domain | `https://openapi.koreainvestment.com:9443` |
| Paper support | Not supported |
| Daytime buy TR ID | `TTTS6036U` |
| Daytime sell TR ID | `TTTS6037U` |
| Revise/cancel API | `/uapi/overseas-stock/v1/trading/daytime-order-rvsecncl` |
| Revise/cancel TR ID | `TTTS6038U` |

Official guide evidence:

- The API summary states that `해외주식 미국주간주문` is for US daytime trading.
- Listed US daytime trading window: Korea time `10:00-18:00`; the API summary says summer time is identical.
- The API summary states that not all US symbols are supported for US daytime trading.

Official sample evidence:

- `daytime_order.py` defines `API_URL = "/uapi/overseas-stock/v1/trading/daytime-order"`.
- For daytime buy orders, the sample selects `TTTS6036U`.
- For daytime sell orders, the sample selects `TTTS6037U`.
- The sample documents `ORD_DVSN = "00"` and says daytime trading supports limit orders only.

Korea Investment guide evidence:

- US daytime trading uses an ATS approved by SEC and FINRA.
- Daytime trading supports buy and sell limit orders only.
- Unfilled daytime orders do not carry into premarket, regular session, or aftermarket; they are automatically canceled before premarket starts.
