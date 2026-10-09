# Deployment build plan

Updated 2026-10-09. Free-tier hosting only; the target is a real application.

Verified locally in the initial audit: Rust workspace checks, 49 Python tests,
13 frontend tests, 46 browser checks, production frontend build and API contract.
The existing server is deliberately local-only; passing these checks does not
make it a publicly deployable application.

1. Add explicit remote mode with a required backend token; retain loopback and
   Host validation for the default local mode. Test both policies.
2. Add a Vercel server-side API bridge with server-only credentials, binary and
   SSE forwarding, timeout/disconnect handling, path restrictions and SPA routing.
3. Build a reproducible backend container and verify its health/startup behavior.
4. Provision a free backend account and persistent model/data artifacts; verify
   cold-start limits and storage before choosing the service.
5. Deploy the frontend to Vercel and run real browser workflows against the hosted
   backend, including forecasting, artifact retrieval and live stream reconnects.
6. Document data provenance, model limitations, rollback and a recruiter demo.

No backend hosting account is confirmed. No paid resources are authorized.
Zex is the current first release priority because it needs no hosted backend.
