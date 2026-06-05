# Rails-to-usagi-api Security Boundary

Status: draft v0.1  
Scope: make `usagi-api` accept production requests only from Rails-controlled infrastructure

---

## 0. Goal

In production, the browser must not call `usagi-api` directly.

`usagi-api` must accept requests only from Rails or approved internal infrastructure.

This is enforced with defense in depth:

```text
private networking
firewall rules
service request signing
request ID propagation
replay protection
production boot checks
optional mTLS later
```

---

## 1. Threat model

### 1.1 Protected assets

```text
source terms
mapping decisions
candidate provenance
engine job results
catalog/search/model artifacts
expensive mapper endpoints
build/index endpoints
```

### 1.2 Threats

| Threat | Mitigation |
|---|---|
| Public browser calls engine directly | network isolation and no public engine ports |
| External attacker calls mapper endpoint | firewall + HMAC signed requests |
| Internal accidental call bypasses Rails | engine signature middleware |
| Request replay | timestamp + nonce cache |
| Body tampering | content SHA256 in signature |
| Secret missing/misconfigured | production boot checks |
| Debug ports left open | deployment smoke test and firewall rules |

---

## 2. Production network rule

Allowed path:

```text
Browser -> Rails -> usagi-api
```

Forbidden path:

```text
Browser -> usagi-api
```

### 2.1 Local development

Docker Compose should use an internal network:

```text
rails-web
  can reach catalog-api/search-api/mapper-api/jobs-api

browser
  can reach rails-web only by default
```

Engine ports may be published only behind an explicit debug override compose file, not the default compose file.

### 2.2 Production

Production should expose only Rails publicly.

```text
public ingress
  -> Rails

private network
  -> catalog-api
  -> search-api
  -> mapper-api
  -> api-worker/jobs
```

Host firewall must deny public access to engine ports.

---

## 3. Request signing

All Rails-to-engine requests include signed headers.

Headers:

```http
X-Request-Id: req_...
X-Usagi-Service: rails
X-Usagi-Timestamp: 2026-05-31T12:00:00Z
X-Usagi-Nonce: nonce_...
X-Usagi-Content-SHA256: hex_sha256_body
X-Usagi-Signature: v1=base64_hmac
```

### 3.1 Canonical string

```text
METHOD
PATH_WITH_QUERY
TIMESTAMP
NONCE
REQUEST_ID
CONTENT_SHA256
```

Example:

```text
POST
/search/concepts
2026-05-31T12:00:00Z
nonce_abc123
req_abc123
e3b0c44298fc1c149afbf4c8996fb924...
```

### 3.2 Signature

```text
base64(HMAC_SHA256(USAGI_API_SHARED_SECRET, canonical_string))
```

Header value:

```text
X-Usagi-Signature: v1=<base64_signature>
```

---

## 4. Rails implementation

### 4.1 Environment variables

```text
USAGI_API_SHARED_SECRET=...
ENGINE_API_TIMEOUT_SECONDS=30
ENGINE_API_OPEN_TIMEOUT_SECONDS=5
CATALOG_API_URL=http://catalog-api:8788
SEARCH_API_URL=http://search-api:8789
MAPPER_API_URL=http://mapper-api:8790
JOBS_API_URL=http://api-worker:8791
```

### 4.2 Request signer

```ruby
module EngineClients
  class RequestSigner
    def initialize(secret: ENV.fetch("USAGI_API_SHARED_SECRET"))
      @secret = secret
    end

    def headers(method:, path_with_query:, body:, request_id:)
      timestamp = Time.now.utc.iso8601
      nonce = "nonce_#{SecureRandom.hex(16)}"
      content_sha = Digest::SHA256.hexdigest(body.to_s)

      canonical = [
        method.to_s.upcase,
        path_with_query,
        timestamp,
        nonce,
        request_id,
        content_sha
      ].join("\n")

      signature = Base64.strict_encode64(
        OpenSSL::HMAC.digest("SHA256", @secret, canonical)
      )

      {
        "X-Request-Id" => request_id,
        "X-Usagi-Service" => "rails",
        "X-Usagi-Timestamp" => timestamp,
        "X-Usagi-Nonce" => nonce,
        "X-Usagi-Content-SHA256" => content_sha,
        "X-Usagi-Signature" => "v1=#{signature}"
      }
    end
  end
end
```

### 4.3 Client rule

All engine clients must use `RequestSigner`.

Do not hand-roll headers inside individual clients. Copy-pasted crypto is where hope goes to become incident response.

---

## 5. API verification

Every `usagi-api` service must verify signed requests in production.

### 5.1 Required config

```text
USAGI_API_AUTH_MODE=signed
USAGI_API_SHARED_SECRET=...
USAGI_API_ALLOWED_CLOCK_SKEW_SECONDS=300
USAGI_API_NONCE_TTL_SECONDS=300
```

Test-only override:

```text
USAGI_API_AUTH_MODE=disabled
```

Production must refuse to boot with `USAGI_API_AUTH_MODE=disabled`.

### 5.2 Verification steps

For every signed endpoint:

```text
1. read required headers
2. reject missing headers
3. parse timestamp
4. reject timestamp outside clock skew window
5. reject nonce already seen within TTL
6. compute SHA256 of body
7. compare body hash with X-Usagi-Content-SHA256
8. rebuild canonical string
9. verify HMAC with constant-time comparison
10. store nonce until TTL expires
11. continue request
```

### 5.3 Endpoint policy

| Endpoint kind | Signing required in production |
|---|---:|
| `/health` liveness | optional if private network only |
| `/status` endpoints | yes |
| search/query endpoints | yes |
| mapper endpoints | yes |
| job endpoints | yes |
| build/index endpoints | yes |

If unsure, require signing. Shocking concept: safer defaults.

---

## 6. Nonce storage

API services need a nonce cache.

MVP options:

```text
in-memory cache per service process
SQLite nonce table
Redis-compatible cache if already deployed
```

Recommended MVP:

```text
in-memory nonce cache for single-process local/dev
SQLite nonce table for production if multiple processes are possible
```

SQLite shape:

```sql
CREATE TABLE request_nonces (
  nonce TEXT PRIMARY KEY,
  seen_at TEXT NOT NULL,
  expires_at TEXT NOT NULL
);

CREATE INDEX idx_request_nonces_expires_at
  ON request_nonces(expires_at);
```

Cleanup expired nonces opportunistically.

---

## 7. Clock skew

Default allowed clock skew:

```text
300 seconds
```

If rejected:

```json
{
  "error": {
    "code": "SIGNATURE_TIMESTAMP_INVALID",
    "message": "Request timestamp is outside the allowed window",
    "request_id": "req_abc"
  }
}
```

Servers must run NTP/time sync. Time bugs are boring until they eat a day.

---

## 8. Error codes

Recommended security error codes:

| Code | Meaning |
|---|---|
| `SIGNATURE_REQUIRED` | signature headers missing |
| `SIGNATURE_VERSION_UNSUPPORTED` | unsupported signature prefix |
| `SIGNATURE_TIMESTAMP_INVALID` | missing/expired/future timestamp |
| `SIGNATURE_NONCE_REPLAYED` | nonce already used |
| `SIGNATURE_BODY_HASH_MISMATCH` | body hash differs |
| `SIGNATURE_INVALID` | HMAC mismatch |
| `AUTH_MODE_DISABLED_IN_PRODUCTION` | unsafe boot config |

All errors should include request ID if available.

---

## 9. Optional mTLS phase

Later add mTLS at reverse proxy or service mesh layer.

```text
Rails client certificate
API reverse proxy verifies certificate
API still verifies HMAC request signature
```

Why both?

```text
mTLS proves transport peer identity
HMAC proves request integrity and replay resistance
```

---

## 10. Logging

Rails logs for engine requests:

```text
request_id
engine_service
method
path
status
engine_request_id
error_code
elapsed_ms
attempt
```

Rails logs safe transport retries as `engine.request.retry` with:

```text
request_id
method
path
attempt
next_attempt
error_class
```

Do not log:

```text
HMAC secret
full signature
full source file contents
large request bodies
raw candidate result artifacts
```

API logs:

```text
request_id
service
signature validation result
error code
caller service
elapsed_ms
```

---

## 11. Production boot checks

Rails production boot must require:

```text
USAGI_API_SHARED_SECRET present
engine URLs present
SECRET_KEY_BASE present
DATABASE_URL present
```

API production boot must require:

```text
USAGI_API_AUTH_MODE=signed
USAGI_API_SHARED_SECRET present
artifact directories configured
job DB configured
```

Production boot must fail loudly if unsafe. A quiet unsafe boot is just a breach with good manners.

---

## 12. Tests

### Rails tests

```text
RequestSigner produces expected headers
content SHA256 matches body
signature changes when body changes
BaseClient sends signed requests
engine errors preserve request_id
missing secret fails production boot
```

### API tests

```text
valid signed request accepted
missing signature rejected
bad signature rejected
expired timestamp rejected
future timestamp rejected
replayed nonce rejected
body hash mismatch rejected
wrong secret rejected
disabled auth refused in production
```

### End-to-end smoke

```text
Rails calls /search/status through private network
API verifies signature
response rendered in admin status page
```

---

## 13. Deployment checklist

Before production:

```text
[ ] only Rails public port is exposed
[ ] engine ports blocked by firewall
[ ] Rails has USAGI_API_SHARED_SECRET
[ ] API has same USAGI_API_SHARED_SECRET
[ ] API auth mode is signed
[ ] health/status smoke test passes
[ ] invalid unsigned request to engine is rejected
[ ] request IDs appear in Rails and API logs
[ ] secrets are not logged
```
