const manifestPath = new URL('./editors/vscode/package.json', import.meta.url);
const lockfilePath = new URL('./editors/vscode/package-lock.json', import.meta.url);

type VersionPart = 'minor' | 'patch';

function nextVersion(version: string, part: VersionPart): string {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(version);
  if (!match) {
    throw new Error(`Expected a semantic version in major.minor.patch form, received: ${version}`);
  }

  const major = Number(match[1]!);
  const minor = Number(match[2]!);
  const patch = Number(match[3]!);
  return part === 'minor' ? `${major}.${minor + 1}.0` : `${major}.${minor}.${patch + 1}`;
}

function parseVersionPart(args: string[]): VersionPart {
  if (args.length === 0) {
    return 'minor';
  }

  if (args.length === 1 && args[0] === '--patch') {
    return 'patch';
  }

  throw new Error('Usage: bun ver [--patch]');
}

const part = parseVersionPart(Bun.argv.slice(2));
const manifest = (await Bun.file(manifestPath).json()) as { version?: unknown };
const lockfile = (await Bun.file(lockfilePath).json()) as {
  version?: unknown;
  packages?: { ''?: { version?: unknown } };
};

if (typeof manifest.version !== 'string') {
  throw new Error('The VS Code extension manifest does not contain a string version.');
}

if (lockfile.version !== manifest.version || lockfile.packages?.['']?.version !== manifest.version) {
  throw new Error('The VS Code extension manifest and lockfile versions are not synchronized.');
}

const previousVersion = manifest.version;
const version = nextVersion(previousVersion, part);
manifest.version = version;
lockfile.version = version;
lockfile.packages['']!.version = version;

await Bun.write(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
await Bun.write(lockfilePath, `${JSON.stringify(lockfile, null, 2)}\n`);

console.log(`${part} version bumped: ${previousVersion} -> ${version}`);
