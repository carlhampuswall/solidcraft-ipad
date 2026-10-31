#!/usr/bin/env python3
"""Compile native vector assets from the canonical SolidCraft icon system.

No artwork or command semantics live here. Xcode compiles the themed SVGs into
Assets.car; UIKit then selects appearance and raster scale and caches images.
Generated files belong in Xcode's target temporary directory, never in the source collection.
"""
import json
import hashlib
import pathlib
import re
import shutil
import sys
import tempfile
import tomllib


def write_changed(path, contents):
    if not path.exists() or path.read_text() != contents:
        path.write_text(contents)


def generate(repository, destination):
    design = repository / 'assets/icons/ui/design'
    sys.path.insert(0, str(design / 'tools'))
    import icon_palette
    palette = tomllib.loads((design / 'palette.toml').read_text())
    aliases = tomllib.loads((design / 'aliases.toml').read_text()).get('alias', {})
    masters = {}
    # Match the upstream resolver: model masters win the historical folder
    # duplicate, while optical small variants are selected separately below.
    for family in ('workspace', 'flat', 'model'):
        for path in sorted((design / 'svg' / family).glob('*.svg')):
            masters[path.stem] = path
    commands = {}
    for path in sorted((repository / 'crates/engine/src/cmd').rglob('*.rs')):
        source = path.read_text()
        declarations = list(re.finditer(r'CommandSpec::new\(\s*"([^"]+)"', source))
        for index, declaration in enumerate(declarations):
            end = declarations[index + 1].start() if index + 1 < len(declarations) else len(source)
            icon = re.search(r'\.icon\(\s*"([^"]+)"\)', source[declaration.end():end])
            if icon:
                commands[declaration[1]] = icon[1]
    destination.mkdir(parents=True, exist_ok=True)
    expected = {'sc-' + name + '.imageset' for name in set(masters) | set(aliases)}
    expected |= {'sc-small-' + name + '.imageset' for name in set(masters) | set(aliases)}
    for stale in destination.glob('sc-*.imageset'):
        if stale.name not in expected:
            shutil.rmtree(stale)
    write_changed(destination / 'Contents.json', json.dumps({'info': {'version': 1, 'author': 'SolidCraft'}}))
    for name in sorted(set(masters) | set(aliases)):
        master, accent = icon_palette.resolve(palette, aliases, name)
        if master not in masters:
            raise ValueError('Missing icon master: ' + name + ' -> ' + master)
        imageset = destination / ('sc-' + name + '.imageset')
        imageset.mkdir(exist_ok=True)
        images = []
        # Keep upstream optical artwork at <=24pt, as well as the full master.
        for small in (False, True):
            variant = design / 'svg/small' / (master + '.svg')
            source = (variant if small and variant.exists() else masters[master]).read_text()
            target = imageset if not small else destination / ('sc-small-' + name + '.imageset')
            target.mkdir(exist_ok=True)
            images = []
            for theme in ('light', 'dark'):
                filename = theme + '.svg'
                svg = icon_palette.themed(source, palette, theme, accent, namespace='native-' + name + '-' + theme)
                # Asset catalogs need a concrete vector size, not just viewBox.
                svg = svg.replace('<svg ', '<svg width="64" height="64" ', 1)
                write_changed(target / filename, svg)
                entry = {'filename': filename, 'idiom': 'universal'}
                if theme == 'dark':
                    entry['appearances'] = [{'appearance': 'luminosity', 'value': 'dark'}]
                images.append(entry)
            write_changed(target / 'Contents.json', json.dumps({
                'images': images, 'info': {'version': 1, 'author': 'SolidCraft'},
                'properties': {'preserves-vector-representation': True}
            }))
    mapping = destination.parent / 'SolidCraftCommandIcons.json'
    write_changed(mapping, json.dumps(commands, sort_keys=True))
    print(f'Native icons: {len(masters)} masters, {len(aliases)} aliases, {len(commands)} command mappings')
    return commands


def check(repository):
    artwork = repository / 'assets/icons/ui/design'
    before = {p: hashlib.sha256(p.read_bytes()).hexdigest() for p in artwork.rglob('*') if p.is_file()}
    with tempfile.TemporaryDirectory(prefix='solidcraft-native-icons-') as temporary:
        output = pathlib.Path(temporary) / 'SolidCraftIcons.xcassets'
        mapping = generate(repository, output)
        native = []
        for area in ('modelling', 'assembly'):
            source = (repository / f'crates/ipados/session/src/{area}.rs').read_text()
            native += re.findall(r'"([^"]+)"', source.split('const TOOLS:', 1)[1].split('];', 1)[0])
        native += ['sketch.point', 'sketch.line', 'sketch.rectangle.two_point', 'sketch.circle.center',
                   'sketch.arc.three_point', 'sketch.spline.fit_point', 'sketch.delete', 'sketch.trim',
                   'sketch.constraint.horizontal_vertical', 'sketch.constraint.parallel',
                   'sketch.constraint.perpendicular', 'sketch.constraint.coincident', 'sketch.constraint.tangent',
                   'sketch.constraint.equal', 'sketch.constraint.concentric', 'sketch.constraint.midpoint',
                   'sketch.constraint.fix']
        for command in native:
            if command not in mapping or not (output / ('sc-' + mapping[command] + '.imageset')).is_dir():
                raise ValueError('Native tool lacks canonical artwork: ' + command)
        for command, expected in [('solid.extrude', 'extrude'), ('sketch.rectangle.two_point', 'rect'),
                                  ('solid.pattern.circular', 'pattern_circ'), ('occurrence.ground', 'ground')]:
            if mapping[command] != expected:
                raise ValueError('Canonical command mapping changed: ' + command)
        for imageset in output.glob('*.imageset'):
            data = json.loads((imageset / 'Contents.json').read_text())
            if not data['properties']['preserves-vector-representation'] or len(data['images']) != 2:
                raise ValueError('Icon lost appearance or vector density support: ' + imageset.name)
            for image in data['images']:
                if not (imageset / image['filename']).is_file():
                    raise ValueError('Missing image rendition')
        stale = output / 'sc-retired.imageset'
        stale.mkdir()
        generate(repository, output)
        if stale.exists():
            raise ValueError('Retired generated artwork survived regeneration')
    if any(hashlib.sha256(p.read_bytes()).hexdigest() != digest for p, digest in before.items()):
        raise ValueError('Native asset generation modified canonical artwork or review metadata')
    print(f'Native icon checks PASS: {len(native)} real tool mappings, vector appearances, regeneration, unchanged masters/review')


if __name__ == '__main__':
    if sys.argv[1] == '--check':
        check(pathlib.Path(sys.argv[2]).resolve())
    else:
        generate(pathlib.Path(sys.argv[1]).resolve(), pathlib.Path(sys.argv[2]).resolve())
