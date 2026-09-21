#!/usr/bin/env python3
"""Draw original standing-fit plans from two live interior surveys.

Inputs are JSON written by the game binary. This does not read source
geometry. Output drawings are measurement diagrams, not reference renders.
"""
import argparse
import json
from collections import defaultdict
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

STOREYS = [-6, 0, 7, 14, 25, 31, 38, 45, 57]
# Half-gap to the neighboring authored storey, so a ramp landing stays on
# one plan instead of painting every floor it can see.
WINDOW = {
    -6: 3.0, 0: 3.0, 7: 3.0, 14: 3.5, 25: 3.0,
    31: 3.0, 38: 3.0, 45: 4.0, 57: 5.0,
}
SCALE = 4
PAD = 28


def cells(base):
    """fit stands keyed by integer (right, forward) -> list of stands."""
    grouped = defaultdict(list)
    for stand in base['stands']:
        if not stand['fits']:
            continue
        key = (round(stand['right']), round(stand['forward']))
        grouped[key].append(stand)
    return grouped


def pick(group, storey):
    window = WINDOW[storey]
    near = [stand for stand in group if abs(stand['floor'] - storey) <= window]
    if not near:
        return None
    return min(near, key=lambda stand: abs(stand['floor'] - storey))


def bounds(groups):
    keys = [key for group in groups for key in group]
    if not keys:
        return -46, 46, -56, 56
    xs = [key[0] for key in keys]
    zs = [key[1] for key in keys]
    return min(xs), max(xs), min(zs), max(zs)


def head_color(stand):
    head = stand['headroom']
    if head >= 20:
        return (186, 196, 168)
    if head >= 8:
        return (214, 168, 74)
    if head < 3.2:
        return (92, 122, 148)
    return (168, 178, 170)


def write_focus(output, ref, ours, x0, x1, z0, z1, font):
    """Larger plans for the storeys whose room shapes disagree."""
    scale = 8
    width = (x1 - x0 + 1) * scale
    height = (z1 - z0 + 1) * scale
    for storey in [0, 14, 25, 31, 38, 45]:
        image = Image.new('RGB', (48 + (width + 20) * 3, height + 92), (8, 12, 18))
        draw = ImageDraw.Draw(image)
        draw.text((16, 10), f'{storey} m standing fit. Flag and entrance are toward the bottom.', fill=(230, 236, 242), font=font)
        draw.rectangle([16, 32, 28, 44], fill=(168, 178, 170))
        draw.text((34, 32), 'enclosed room', fill=(180, 188, 196), font=font)
        draw.rectangle([150, 32, 162, 44], fill=(214, 168, 74))
        draw.text((168, 32), 'tall void above', fill=(180, 188, 196), font=font)
        draw.rectangle([310, 32, 322, 44], fill=(70, 170, 190))
        draw.text((328, 32), 'reference only', fill=(180, 188, 196), font=font)
        draw.rectangle([470, 32, 482, 44], fill=(210, 120, 64))
        draw.text((488, 32), 'Skybreak only', fill=(180, 188, 196), font=font)
        draw.rectangle([630, 32, 642, 44], fill=(214, 196, 64))
        draw.text((648, 32), 'ceiling differs', fill=(180, 188, 196), font=font)
        panels = []
        for group, kind in ((ref, 'ref'), (ours, 'sky')):
            colors = {}
            for key, stands in group.items():
                stand = pick(stands, storey)
                if stand is not None:
                    colors[key] = head_color(stand)
            panels.append(colors)
        diff = {}
        for key, color in panels[0].items():
            if key not in panels[1]:
                diff[key] = (70, 170, 190)
            elif abs(pick(ref[key], storey)['headroom'] - pick(ours[key], storey)['headroom']) > 2.5:
                diff[key] = (214, 196, 64)
            else:
                diff[key] = (70, 78, 86)
        for key in panels[1]:
            if key not in panels[0]:
                diff[key] = (210, 120, 64)
        panels.append(diff)
        for index, (title, colors) in enumerate(zip(('Reference app', 'Skybreak', 'Difference'), panels)):
            ox = 16 + index * (width + 20)
            oy = 64
            draw.text((ox, oy - 16), title, fill=(220, 226, 232), font=font)
            draw.rectangle([ox, oy, ox + width, oy + height], fill=(16, 22, 30))
            for (right, forward), color in colors.items():
                if not (x0 <= right <= x1 and z0 <= forward <= z1):
                    continue
                px = ox + (right - x0) * scale
                py = oy + (z1 - forward) * scale
                draw.rectangle([px, py, px + scale - 1, py + scale - 1], fill=color)
            # 10 m scale bar along the left edge of the first panel only.
            if index == 0:
                bar = ox + 8
                by = oy + height - 16
                draw.line([bar, by, bar + 10 * scale, by], fill=(230, 236, 242), width=2)
                draw.text((bar, by - 14), '10 m', fill=(230, 236, 242), font=font)
        image.save(output / f'storey-{storey}.png')


def write_split_sections(output, ref, ours, font):
    image = Image.new('RGB', (980, 760), (8, 12, 18))
    draw = ImageDraw.Draw(image)
    draw.text((16, 8), 'Centerline floors. Top is the reference app, bottom is Skybreak. Stem length is headroom.', fill=(230, 236, 242), font=font)

    def plot(group, top, color):
        for (right, forward), stands in group.items():
            if right != 0:
                continue
            for stand in stands:
                if not (-56 <= forward <= 48 and -16 <= stand['floor'] <= 70):
                    continue
                px = 70 + int((forward + 56) * 8)
                py = top + int((70 - stand['floor']) * 4)
                stem = min(stand['headroom'], 14) * 4
                draw.line([px, py, px, py - stem], fill=color, width=3)
                draw.ellipse([px - 2, py - 2, px + 2, py + 2], fill=color)

    for top, group, label in ((40, ref, 'Reference'), (400, ours, 'Skybreak')):
        draw.text((16, top), label, fill=(180, 188, 196), font=font)
        for level in STOREYS:
            py = top + 28 + int((70 - level) * 4)
            draw.line([60, py, 940, py], fill=(36, 44, 54))
            draw.text((16, py - 6), str(level), fill=(120, 130, 140), font=font)
        plot(group, top + 28, (120, 186, 194) if group is ref else (214, 140, 78))
        for forward, name in ((-40, 'entrance'), (-12, 'flag'), (12, 'hall'), (36, 'rear')):
            px = 70 + int((forward + 56) * 8)
            draw.text((px - 10, top + 16), name, fill=(140, 150, 160), font=font)
    image.save(output / 'centerline.png')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference', type=Path, required=True)
    parser.add_argument('--skybreak', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--base', type=int, default=0)
    args = parser.parse_args()
    reference = json.loads(args.reference.read_text())
    skybreak = json.loads(args.skybreak.read_text())
    ref = cells(reference['bases'][args.base])
    ours = cells(skybreak['bases'][args.base])
    x0, x1, z0, z1 = bounds([ref, ours])
    # Keep the authored hull in frame even if one side is sparse.
    x0, x1 = min(x0, -40), max(x1, 40)
    z0, z1 = min(z0, -48), max(z1, 44)
    width = (x1 - x0 + 1) * SCALE
    height = (z1 - z0 + 1) * SCALE
    font = ImageFont.load_default()
    header = 36
    row_h = height + header + 8
    image = Image.new('RGB', (PAD + (width + 16) * 3, PAD + row_h * len(STOREYS) + 24), (8, 12, 18))
    draw = ImageDraw.Draw(image)
    titles = ['Reference app, where a player fits', 'Skybreak, where a player fits', 'Only one side, or ceiling differs']
    for index, title in enumerate(titles):
        draw.text((PAD + index * (width + 16), 8), title, fill=(220, 226, 232), font=font)
    summary = []
    for row, storey in enumerate(STOREYS):
        y = PAD + row * row_h
        draw.text((8, y), f'{storey} m', fill=(220, 226, 232), font=font)
        both = only_ref = only_ours = ceiling = 0
        ref_n = ours_n = 0
        ref_colors = {}
        ours_colors = {}
        diff = {}
        for key, stands in ref.items():
            stand = pick(stands, storey)
            if stand is None:
                continue
            ref_n += 1
            ref_colors[key] = head_color(stand)
            twin = pick(ours[key], storey) if key in ours else None
            if twin is None:
                only_ref += 1
                diff[key] = (70, 170, 190)
            elif abs(stand['headroom'] - twin['headroom']) > 2.5:
                ceiling += 1
                diff[key] = (214, 196, 64)
            else:
                both += 1
                diff[key] = (70, 78, 86)
        for key, stands in ours.items():
            stand = pick(stands, storey)
            if stand is None:
                continue
            ours_n += 1
            ours_colors[key] = head_color(stand)
            if pick(ref[key], storey) is None if key in ref else True:
                only_ours += 1
                diff[key] = (210, 120, 64)

        def blit(index, colors):
            ox = PAD + index * (width + 16)
            oy = y + header
            draw.rectangle([ox, oy, ox + width, oy + height], fill=(16, 22, 30))
            for (right, forward), color in colors.items():
                if not (x0 <= right <= x1 and z0 <= forward <= z1):
                    continue
                px = ox + (right - x0) * SCALE
                py = oy + (z1 - forward) * SCALE
                draw.rectangle([px, py, px + SCALE - 1, py + SCALE - 1], fill=color)

        blit(0, ref_colors)
        blit(1, ours_colors)
        blit(2, diff)
        line = (f'{storey:3} m  ref {ref_n:4}  sky {ours_n:4}  '
                f'both {both:4}  reference-only {only_ref:4}  '
                f'skybreak-only {only_ours:4}  ceiling>{2.5} {ceiling:4}')
        summary.append(line)
        print(line)
    legend = 'Cyan: reference only. Orange: Skybreak only. Yellow: both, headroom differs by more than 2.5 m. Gray: both similar. Plans: gold is a tall void, pale is open sky, blue is a low ceiling.'
    draw.text((PAD, image.height - 18), legend[:180], fill=(180, 188, 196), font=font)
    args.output.mkdir(parents=True, exist_ok=True)
    image.save(args.output / 'storeys.png')

    # Centerline and gallery sections: floor height vs forward.
    section = Image.new('RGB', (1100, 720), (8, 12, 18))
    sd = ImageDraw.Draw(section)
    sd.text((16, 8), 'Center (right=0) and gallery (right=-15). Dot is a floor; stem is headroom, capped at 12 m.', fill=(220, 226, 232), font=font)

    def plot(group, x_shift, color, right_target):
        for (right, forward), stands in group.items():
            if abs(right - right_target) > 0:
                continue
            for stand in stands:
                if not (-56 <= forward <= 56 and -20 <= stand['floor'] <= 70):
                    continue
                px = 80 + x_shift + int((forward + 56) * 7)
                py = 640 - int((stand['floor'] + 20) * 6)
                stem = min(stand['headroom'], 12) * 6
                sd.line([px, py, px, py - stem], fill=color, width=2)
                sd.ellipse([px - 2, py - 2, px + 2, py + 2], fill=color)

    sd.text((80, 36), 'right = 0', fill=(180, 188, 196), font=font)
    sd.text((620, 36), 'right = -15', fill=(180, 188, 196), font=font)
    plot(ref, 0, (90, 190, 200), 0)
    plot(ours, 0, (220, 130, 70), 0)
    plot(ref, 540, (90, 190, 200), -15)
    plot(ours, 540, (220, 130, 70), -15)
    for forward, label in [(-40, 'entrance'), (-12, 'flag'), (10, 'hall'), (30, 'rear')]:
        px = 80 + int((forward + 56) * 7)
        sd.line([px, 60, px, 660], fill=(40, 48, 58))
        sd.text((px + 2, 48), label, fill=(140, 150, 160), font=font)
    for level in STOREYS:
        py = 640 - int((level + 20) * 6)
        sd.line([70, py, 1060, py], fill=(36, 44, 54))
        sd.text((16, py - 6), str(level), fill=(140, 150, 160), font=font)
    section.save(args.output / 'sections.png')
    write_focus(args.output, ref, ours, x0, x1, z0, z1, font)
    write_split_sections(args.output, ref, ours, font)

    print('\nProbes, base', args.base, '(overlay ray, metres; None is a miss within 150)')
    ref_base = reference['bases'][args.base]
    our_base = skybreak['bases'][args.base]
    ours_by_name = {probe['name']: probe for probe in our_base['probes']}
    for probe in ref_base['probes']:
        other = ours_by_name.get(probe['name'])
        if other is None:
            continue
        bits = [probe['name']]
        for axis in ['left', 'right', 'back', 'front', 'floor', 'ceiling']:
            a = probe['rays'][axis]
            b = other['rays'][axis]
            if a is None or b is None:
                bits.append(f'{axis} ref {a} sky {b}')
            else:
                bits.append(f'{axis} {b - a:+.2f}')
        print('  ' + '  '.join(bits))
    (args.output / 'summary.txt').write_text('\n'.join(summary) + '\n')
    print('wrote', args.output / 'storeys.png')


if __name__ == '__main__':
    main()
