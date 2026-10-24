"""Compara los overrides instalados con el ZIP exacto publicado en CurseForge, por tamaño y CRC32."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import urllib.request
from urllib.parse import urlparse
import zipfile
import zlib

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--instance', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
bridge = 'https://curseforge-86460886589.us-central1.run.app'
request = urllib.request.Request(bridge + '/v1/mods/1298402/files/9261952', headers={'User-Agent':'Parabellum-QA/0.39.1'})
with urllib.request.urlopen(request, timeout=40) as response:
    metadata = json.load(response)['data']
assert metadata['modId'] == 1298402 and metadata['id'] == 9261952
assert metadata['fileLength'] < 200_000_000
url = urlparse(metadata['downloadUrl'])
assert url.scheme == 'https' and url.hostname in {'edge.forgecdn.net', 'mediafilez.forgecdn.net', 'media.forgecdn.net'}
args.output.mkdir(parents=True, exist_ok=True)
archive = args.output / 'ATM10SKY-2.2.0-oficial.zip'
with urllib.request.urlopen(metadata['downloadUrl'], timeout=90) as response, archive.open('wb') as stream:
    received = 0
    while chunk := response.read(65536):
        received += len(chunk)
        if received > metadata['fileLength']:
            raise ValueError('La respuesta excede el tamaño oficial del ZIP')
        stream.write(chunk)
assert archive.stat().st_size == metadata['fileLength']
with archive.open('rb') as stream:
    sha1 = hashlib.file_digest(stream, 'sha1').hexdigest()
assert sha1 == next(h['value'].lower() for h in metadata['hashes'] if h['algo'] == 1)
checked, changed, ignored = [], [], []
with zipfile.ZipFile(archive) as pack:
    manifest = json.loads(pack.read('manifest.json'))
    prefix = manifest['overrides'].strip('/') + '/'
    for entry in pack.infolist():
        if entry.is_dir() or not entry.filename.startswith(prefix):
            continue
        relative = PurePosixPath(entry.filename[len(prefix):])
        assert not relative.is_absolute() and '..' not in relative.parts
        if str(relative) == 'options.txt':
            ignored.append('options.txt: el instalador ajusta el idioma')
            continue
        target = args.instance.joinpath(*relative.parts)
        assert target.resolve().is_relative_to(args.instance.resolve())
        if not target.is_file() or target.stat().st_size != entry.file_size:
            changed.append(str(relative))
            continue
        crc = 0
        with target.open('rb') as stream:
            while chunk := stream.read(65536):
                crc = zlib.crc32(chunk, crc)
        (checked if crc == entry.CRC else changed).append(str(relative))
report = {'project':'cf:1298402','version':'cf:9261952','archiveSha1':sha1,
          'matching':len(checked),'changed':changed,'ignored':ignored,'manifestFiles':len(manifest['files'])}
(args.output/'overrides.json').write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
print(json.dumps(report, ensure_ascii=False, indent=2))
if changed:
    raise SystemExit(1)
