# Optional private evidence capture

The recorder and durable handoff utilities capture locally owned game output for private inspection. They do not edit, narrate or publish videos. Copy `config.example.json` to a private ignored configuration, fill your own paths and select it with `D1_CONTENT_CONFIG`. Leave capture disabled unless you deliberately configure it.

Supply a separately installed FFmpeg. Use existing operating-system access to your own storage; do not embed credentials in configuration. Recordings, screenshots, raw logs, outboxes and machine configuration must remain outside Git. Full decoding or successful transfer is not privacy or rights clearance. Actual picture, audio and source provenance require review before any publication.

Run the tests described in the root README against disposable fixtures. Capture failures preserve source evidence and must not be presented as successful recordings or complete gameplay acceptance.
