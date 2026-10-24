"""Package a verified Windows standalone binary without copying private workspaces."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def package(binary, output):
    binary = binary.resolve(strict=True)
    if binary.suffix.lower() != '.exe':
        raise ValueError('This release packager expects the Windows x64 executable')
    output = output.resolve()
    if output.exists():
        raise ValueError('Output directory already exists; choose a new release directory')
    archive = output.parent / (output.name + '.zip')
    checksum = archive.with_suffix('.zip.sha256')
    if archive.exists() or checksum.exists():
        raise ValueError('Release archive or checksum already exists')
    checked = subprocess.run([str(binary), '--check'], check=True, capture_output=True,
                             text=True, timeout=30)
    verified = json.loads(checked.stdout)
    manifest = json.loads((ROOT / 'deploy/artifacts.tar.json').read_text(encoding='utf-8'))
    if verified != {'status': 'verified', 'source': manifest['source'], 'model': manifest['model'],
                    'dataset': manifest['dataset'], 'symbol': manifest['symbol'],
                    'bundle_sha256': manifest['bundle_sha256'], 'files': len(manifest['files'])}:
        raise ValueError('Executable does not match the selected artifact bundle')
    revision = subprocess.run(['git', 'rev-parse', 'HEAD'], cwd=ROOT, check=True,
                              capture_output=True, text=True).stdout.strip()
    dirty = bool(subprocess.run(['git', 'status', '--porcelain', '--untracked-files=normal'],
                               cwd=ROOT, check=True, capture_output=True, text=True).stdout.strip())
    files = {'lstm-inspector.exe': binary.read_bytes(),
             'START-HERE.md': (ROOT / 'Docs/24-PORTABLE-APP.md').read_bytes().replace(
                 b'(25-PORTABLE-VERIFICATION.md)', b'(VERIFICATION.md)'),
             'VERIFICATION.md': (ROOT / 'Docs/25-PORTABLE-VERIFICATION.md').read_bytes()}
    info = {'schema_version': 1, 'platform': 'windows-x64', 'source_revision': revision,
            'working_tree_dirty': dirty, 'data_source': manifest['source'],
            'embedded_bundle_sha256': manifest['bundle_sha256'],
            'files': {name: {'sha256': sha256(data), 'size_bytes': len(data)}
                      for name, data in files.items()}}
    files['release.json'] = (json.dumps(info, indent=2) + '\n').encode()
    output.mkdir(parents=True)
    for name, data in files.items():
        (output / name).write_bytes(data)
    with archive.open('xb') as stream, zipfile.ZipFile(stream, 'w', compression=zipfile.ZIP_DEFLATED) as zipped:
        for name, data in sorted(files.items()):
            item = zipfile.ZipInfo(output.name + '/' + name, (1980, 1, 1, 0, 0, 0))
            item.compress_type = zipfile.ZIP_DEFLATED
            zipped.writestr(item, data)
    with checksum.open('x', encoding='utf-8') as stream:
        stream.write(f'{sha256(archive.read_bytes())}  {archive.name}\n')
    print(json.dumps({'archive': str(archive), 'sha256': sha256(archive.read_bytes()),
                      'binary_bytes': len(files['lstm-inspector.exe']), 'source_revision': revision,
                      'working_tree_dirty': dirty}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    package(**vars(parser.parse_args()))
