import esbuild from "esbuild";

esbuild
  .build({
    entryPoints: ["src/app.css"],
    mainFields: ["browser", "module", "main"],
    bundle: true,
    minify: true,
    outfile: "build/bundle.css",
    logLevel: "info",
  })
  .catch((error, location) => {
    console.warn(`Errors: `, error, location);
    process.exit(1)
  });
