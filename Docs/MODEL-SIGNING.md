# Optional model authenticity

Hash-only mode remains the default. It detects changes relative to `integrity.json` but cannot authenticate a manifest that an attacker rewrites alongside package files.

To require authenticity, deploy a protected YAML policy selected by the `QUANTCTL_MODEL_TRUST_CONFIG` environment variable:

```yaml
require_signature: true
public_key_hex: YOUR_TRUSTED_ED25519_PUBLIC_KEY_HEX
```

The public key must be provisioned through trusted configuration, not accepted from the model directory. The policy applies to package verification and therefore model loading in prediction, replay, forecast, monitoring and the Rust benchmark. Missing, malformed, tampered, wrong-key or unsigned packages fail when required. A signed package presented without a trusted configured public key also fails; it is not silently treated as authenticated.

Sign only after training publication and hash verification. Supply the hex-encoded Ed25519 PKCS8 v2 private key through `QUANTCTL_MODEL_SIGNING_KEY_PKCS8_HEX` in the signing process environment, then run:

```powershell
quantctl sign-model --artifact-dir models/MODEL
```

The command creates a detached `integrity.ed25519` signature over the exact manifest bytes and refuses to overwrite an existing signature. It prints only the public key. Do not commit a private key, put it on the command line, or paste it into chat/evidence. Provision keys through your secret manager; this repository does not generate or retain production signing keys.

The signer needs a stable, exclusively controlled artifact directory. Signatures do not prevent replacement between verification and later file reads; preserve immutable deployment directories and filesystem access controls. Signing does not certify data quality, predictive performance, or the identity of a key whose trust configuration was itself replaced. Key rotation requires a deliberate trusted-policy update and newly signed deployment artifacts.

Implementation uses the existing `ring` dependency's [Ed25519 key and verification APIs](https://docs.rs/ring/0.17.14/ring/signature/index.html). The staged export parity verifier remains a numerical development gate; it is not a signature acceptance command for deployment.
