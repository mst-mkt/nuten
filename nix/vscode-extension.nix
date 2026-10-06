{
  lib,
  vscode-utils,
  fetchPnpmDeps,
  pnpmConfigHook,
  esbuild,
  pnpm,
}:
let
  manifest = lib.importJSON ../integrations/vscode/package.json;
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../package.json
      ../pnpm-lock.yaml
      ../pnpm-workspace.yaml
      ../LICENSE
      ../assets/logo.png
      ../integrations/vscode/package.json
      ../integrations/vscode/icon.png
      ../integrations/vscode/LICENSE
      (lib.fileset.difference ../integrations/vscode/src ../integrations/vscode/src/test)
    ];
  };
in
vscode-utils.buildVscodeExtension (finalAttrs: {
  pname = "${manifest.publisher}-${manifest.name}";
  inherit (manifest) version;
  vscodeExtPublisher = manifest.publisher;
  vscodeExtName = manifest.name;
  vscodeExtUniqueId = "${manifest.publisher}.${manifest.name}";

  inherit src;
  sourceRoot = src.name;

  pnpmWorkspaces = [ manifest.name ];
  pnpmInstallFlags = [ "--prod" ];
  pnpmDeps = fetchPnpmDeps {
    inherit (finalAttrs)
      pname
      version
      src
      pnpmWorkspaces
      pnpmInstallFlags
      ;
    inherit pnpm;
    fetcherVersion = 4;
    hash = "sha256-AmuKnNoq0ajzctosyzZUjDL4KOjN9Q3Potx3dO2fhfw=";
  };

  nativeBuildInputs = [
    pnpm
    pnpmConfigHook
    esbuild
  ];

  buildPhase = ''
    runHook preBuild
    cd integrations/vscode
    esbuild src/extension.ts --bundle --platform=node --format=cjs --external:vscode --minify --outfile=${manifest.main}
    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall
    mkdir -p "$out/$installPrefix"
    cp --dereference --parents --target-directory="$out/$installPrefix" package.json ${lib.escapeShellArgs manifest.files}
    runHook postInstall
  '';

  meta = {
    inherit (manifest) description;
    homepage = "https://github.com/mst-mkt/nuten";
    license = lib.licenses.mit;
    sourceProvenance = [ lib.sourceTypes.fromSource ];
  };
})
