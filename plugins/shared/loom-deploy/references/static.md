# Static Deployment Reference

Use this reference when implementing or repairing static-site deployment support.

## Scanner Signals

- Static output directories: `dist`, `build`, `public`, `out`, `_site`.
- Plain static entry files: `index.html`, `404.html`, static assets without a server entrypoint.
- Node projects with Vite/Astro/Next export may still use the Node reference first when `package.json` has build/preview scripts.

## Template Rules

- For already-built static assets, use an Nginx runtime image and copy the detected output directory into the default web root.
- If a build step is detected, use the stack-specific build stage first, then copy the output directory into Nginx.
- Default container port is `80`.
- Add SPA fallback only when the framework or project signals client-side routing. Do not force fallback for plain static docs.

## Repair Notes

- Common failures are missing build output, wrong output directory, generated Nginx config not copied, or a project that needs a build command before static serving.
- If the project is a library/docs source without generated assets, report the missing build artifact instead of serving the source tree blindly.
