#!/usr/bin/env python3
"""Generate the Tauri command wrappers of the src-tauri adapter from the v2 handler list
and the (Tauri-free) core command signatures."""
import re, sys, pathlib, json
ROOT = pathlib.Path('/home/arch/Project/test/wt-v3-core')
SRC = ROOT / 'crates/rustsale-core/src'
orig = open('/tmp/orig_lib.rs').read()
a = orig.index('tauri::generate_handler![') + len('tauri::generate_handler![')
b = orig.index(']', a)
entries = [e.strip() for e in orig[a:b].split(',') if e.strip()]

def find_file(parts):
    mod = parts[:-1]
    cands = [SRC.joinpath(*mod).with_suffix('.rs'), SRC.joinpath(*mod, 'mod.rs')]
    if not mod:
        cands = [SRC / 'lib.rs']
    for c in cands:
        if c.exists():
            return c
    raise SystemExit(f'no file for {parts}')

def balanced(s, i, open_c='(', close_c=')'):
    depth = 0
    j = i
    while j < len(s):
        c = s[j]
        if c == open_c: depth += 1
        elif c == close_c:
            depth -= 1
            if depth == 0: return j
        j += 1
    raise SystemExit('unbalanced')

def split_top(s):
    out, depth, cur = [], 0, ''
    pairs = {'<': '>', '(': ')', '[': ']'}
    i = 0
    while i < len(s):
        c = s[i]
        if c == '-' and s[i+1:i+2] == '>':
            cur += '->'; i += 2; continue
        if c in '<([': depth += 1
        elif c in '>)]': depth -= 1
        if c == ',' and depth == 0:
            out.append(cur); cur = ''
        else:
            cur += c
        i += 1
    if cur.strip(): out.append(cur)
    return [x.strip() for x in out if x.strip()]

def parse_fn(path, name):
    s = path.read_text()
    m = re.search(r'^pub (async )?fn ' + re.escape(name) + r'\s*(<[^>]*>)?\s*\(', s, re.M)
    if not m:
        raise SystemExit(f'fn {name} not found in {path}')
    start = m.end() - 1
    end = balanced(s, start)
    params = split_top(s[start+1:end])
    rest = s[end+1:]
    brace = rest.index('{')
    ret = rest[:brace].strip()
    if ret.startswith('->'): ret = ret[2:].strip()
    else: ret = ''
    if ' where ' in ret: ret = ret.split(' where ')[0].strip()
    return bool(m.group(1)), params, ret, m.group(2), s

def uses_of(s, modpath):
    # top level use statements before any `#[cfg(test)]`
    head = s.split('\n#[cfg(test)]')[0]
    uses = re.findall(r'^(?:pub(?:\([a-z]+\))? )?use [^;]*;', head, re.M | re.S)
    out = []
    for u in uses:
        u = re.sub(r'^pub(\([a-z]+\))? ', '', u)
        u = re.sub(r'\bcrate::', 'rustsale_core::', u)
        base = 'rustsale_core::' + '::'.join(modpath[:-1] if True else modpath)
        u = re.sub(r'\buse super::', 'use ' + 'rustsale_core::' + '::'.join(modpath[:-2]) + ('::' if len(modpath) > 2 else ''), u) if modpath[:-1] else u
        u = re.sub(r'\buse self::', 'use ' + base + '::', u)
        out.append(u)
    return out

mods = {}
report = []
for e in entries:
    parts = e.split('::')
    name = parts[-1]
    f = find_file(parts)
    is_async, params, ret, generics, src = parse_fn(f, name)
    key = tuple(parts[:-1])
    mods.setdefault(key, {'src': src, 'cmds': []})['cmds'].append((name, is_async, params, ret, generics))

def render_module(key, info):
    modpath = list(key)
    lines = []
    lines.append('#![allow(unused_imports, unused_variables, clippy::too_many_arguments, clippy::type_complexity)]')
    lines.append('use crate::CoreHandle;')
    lines.append('use rustsale_core::host::{State, AppHandle};')
    if modpath:
        lines.append('#[allow(unused_imports)]\nuse rustsale_core::%s::*;' % '::'.join(modpath))
    for u in uses_of(info['src'], modpath + ['x']):
        if 'rustsale_core::host' in u:
            continue
        root = re.match(r'use (?:::)?([A-Za-z_0-9]+)', u).group(1)
        if root not in ('rustsale_core', 'std', 'serde_json', 'serde', 'chrono'):
            continue
        lines.append(u)
    lines.append('')
    for name, is_async, params, ret, generics in info['cmds']:
        sig_params = ['core: tauri::State<\'_, CoreHandle>']
        pre = ['let app = core.handle();']
        call_args = []
        is_result = ret.startswith('Result<')
        ret = re.sub(r'\bcrate::', 'rustsale_core::', ret)
        for p in params:
            p = re.sub(r'\bcrate::', 'rustsale_core::', p)
            pname, ptype = [x.strip() for x in p.split(':', 1)]
            pname = (pname[4:] if pname.startswith('mut ') else pname).strip()
            if ptype == 'AppHandle':
                call_args.append('app.clone()')
            elif re.match(r"State<", ptype):
                inner = re.match(r"State<\s*(?:'_\s*,\s*)?(.*)>$", ptype).group(1)
                vname = 'st_' + pname.lstrip('_')
                rr = ret.replace('\n', ' ')
                m_err = re.match(r'Result<.*,\s*([A-Za-z_:]+)>$', rr)
                if m_err and m_err.group(1) == 'String':
                    pre.append(f'let {vname} = app.state::<{inner}>().map_err(|e| e.to_string())?;')
                elif m_err and m_err.group(1) == 'ConversationCommandError':
                    pre.append(f'let {vname} = app.state::<{inner}>().map_err(|e| rustsale_core::conversation::error::ConversationError::new(rustsale_core::conversation::error::ConversationErrorCode::ProviderFailure, e.to_string()).command_error())?;')
                elif m_err:
                    raise SystemExit(f'unhandled error type {m_err.group(1)} in {name}')
                else:
                    pre.append(f'let {vname} = match app.state::<{inner}>() {{ Ok(s) => s, Err(e) => {{ tracing::error!(target: "tauri_adapter", "state unavailable: {{e}}"); return Default::default(); }} }};')
                call_args.append(vname)
            elif ptype.startswith('Channel<') or ptype.startswith('rustsale_core::host::Channel<'):
                inner = re.match(r'(?:rustsale_core::host::)?Channel<(.*)>$', ptype).group(1)
                sig_params.append(f'{pname}: tauri::ipc::Channel<{inner}>')
                call_args.append(f'rustsale_core::host::Channel::new(move |v| {pname}.send(v).map_err(|e| rustsale_core::host::PortError::failed(e.to_string())))')
            else:
                sig_params.append(f'{pname}: {ptype}')
                call_args.append(pname)
        path = '::'.join(modpath + [name]) if modpath else name
        full = ('rustsale_core::' + path) if modpath else f'rustsale_core::{name}'
        call = f'{full}({", ".join(call_args)})' + ('.await' if is_async else '')
        retsig = f' -> {ret}' if ret else ''
        lines.append('#[tauri::command]')
        lines.append(f'pub {"async " if is_async else ""}fn {name}({", ".join(sig_params)}){retsig} {{')
        for l in pre: lines.append('    ' + l)
        lines.append('    ' + call)
        lines.append('}\n')
    return '\n'.join(lines)

outdir = ROOT / 'src-tauri/src/wrappers'
outdir.mkdir(parents=True, exist_ok=True)
names = []
for key, info in mods.items():
    fname = '_'.join(key) if key else 'root'
    (outdir / f'{fname}.rs').write_text(render_module(key, info))
    names.append((fname, key, [c[0] for c in info['cmds']]))
with open(outdir / 'mod.rs', 'w') as fh:
    fh.write('//! GENERATED by scratch-tools/gen_wrappers.py: Tauri command wrappers over rustsale-core.\n')
    for fname, _, _ in names:
        fh.write(f'pub mod {fname};\n')
    fh.write('\n#[macro_export]\nmacro_rules! handler_list {\n    () => {\n        tauri::generate_handler![\n')
    for fname, key, cmds in names:
        for c in cmds:
            fh.write(f'            $crate::wrappers::{fname}::{c},\n')
    fh.write('        ]\n    };\n}\n')
print(len(entries), 'entries,', len(mods), 'modules')
