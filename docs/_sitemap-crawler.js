function be(e) {
  return typeof e > 'u' || e === null;
}
function mn(e) {
  return typeof e == 'object' && e !== null;
}
function xn(e) {
  return Array.isArray(e) ? e : be(e) ? [] : [e];
}
function gn(e, n) {
  var i, l, r, u;
  if (n) for (u = Object.keys(n), i = 0, l = u.length; i < l; i += 1) (r = u[i]), (e[r] = n[r]);
  return e;
}
function An(e, n) {
  var i = '',
    l;
  for (l = 0; l < n; l += 1) i += e;
  return i;
}
function vn(e) {
  return e === 0 && Number.NEGATIVE_INFINITY === 1 / e;
}
var yn = be,
  wn = mn,
  Cn = xn,
  _n = An,
  En = vn,
  Sn = gn,
  C = { isNothing: yn, isObject: wn, toArray: Cn, repeat: _n, isNegativeZero: En, extend: Sn };
function Te(e, n) {
  var i = '',
    l = e.reason || '(unknown reason)';
  return e.mark
    ? (e.mark.name && (i += 'in "' + e.mark.name + '" '),
      (i += '(' + (e.mark.line + 1) + ':' + (e.mark.column + 1) + ')'),
      !n &&
        e.mark.snippet &&
        (i +=
          `

` + e.mark.snippet),
      l + ' ' + i)
    : l;
}
function U(e, n) {
  Error.call(this),
    (this.name = 'YAMLException'),
    (this.reason = e),
    (this.mark = n),
    (this.message = Te(this, !1)),
    Error.captureStackTrace ? Error.captureStackTrace(this, this.constructor) : (this.stack = new Error().stack || '');
}
U.prototype = Object.create(Error.prototype);
U.prototype.constructor = U;
U.prototype.toString = function (n) {
  return this.name + ': ' + Te(this, n);
};
var b = U;
function z(e, n, i, l, r) {
  var u = '',
    o = '',
    c = Math.floor(r / 2) - 1;
  return (
    l - n > c && ((u = ' ... '), (n = l - c + u.length)),
    i - l > c && ((o = ' ...'), (i = l + c - o.length)),
    { str: u + e.slice(n, i).replace(/\t/g, '⟶') + o, pos: l - n + u.length }
  );
}
function J(e, n) {
  return C.repeat(' ', n - e.length) + e;
}
function Fn(e, n) {
  if (((n = Object.create(n || null)), !e.buffer)) return null;
  n.maxLength || (n.maxLength = 79),
    typeof n.indent != 'number' && (n.indent = 1),
    typeof n.linesBefore != 'number' && (n.linesBefore = 3),
    typeof n.linesAfter != 'number' && (n.linesAfter = 2);
  for (var i = /\r?\n|\r|\0/g, l = [0], r = [], u, o = -1; (u = i.exec(e.buffer)); )
    r.push(u.index), l.push(u.index + u[0].length), e.position <= u.index && o < 0 && (o = l.length - 2);
  o < 0 && (o = l.length - 1);
  var c = '',
    f,
    a,
    p = Math.min(e.line + n.linesAfter, r.length).toString().length,
    t = n.maxLength - (n.indent + p + 3);
  for (f = 1; f <= n.linesBefore && !(o - f < 0); f++)
    (a = z(e.buffer, l[o - f], r[o - f], e.position - (l[o] - l[o - f]), t)),
      (c =
        C.repeat(' ', n.indent) +
        J((e.line - f + 1).toString(), p) +
        ' | ' +
        a.str +
        `
` +
        c);
  for (
    a = z(e.buffer, l[o], r[o], e.position, t),
      c +=
        C.repeat(' ', n.indent) +
        J((e.line + 1).toString(), p) +
        ' | ' +
        a.str +
        `
`,
      c +=
        C.repeat('-', n.indent + p + 3 + a.pos) +
        `^
`,
      f = 1;
    f <= n.linesAfter && !(o + f >= r.length);
    f++
  )
    (a = z(e.buffer, l[o + f], r[o + f], e.position - (l[o] - l[o + f]), t)),
      (c +=
        C.repeat(' ', n.indent) +
        J((e.line + f + 1).toString(), p) +
        ' | ' +
        a.str +
        `
`);
  return c.replace(/\n$/, '');
}
var bn = Fn,
  Tn = ['kind', 'multi', 'resolve', 'construct', 'instanceOf', 'predicate', 'represent', 'representName', 'defaultStyle', 'styleAliases'],
  On = ['scalar', 'sequence', 'mapping'];
function Nn(e) {
  var n = {};
  return (
    e !== null &&
      Object.keys(e).forEach(function (i) {
        e[i].forEach(function (l) {
          n[String(l)] = i;
        });
      }),
    n
  );
}
function In(e, n) {
  if (
    ((n = n || {}),
    Object.keys(n).forEach(function (i) {
      if (Tn.indexOf(i) === -1) throw new b('Unknown option "' + i + '" is met in definition of "' + e + '" YAML type.');
    }),
    (this.options = n),
    (this.tag = e),
    (this.kind = n.kind || null),
    (this.resolve =
      n.resolve ||
      function () {
        return !0;
      }),
    (this.construct =
      n.construct ||
      function (i) {
        return i;
      }),
    (this.instanceOf = n.instanceOf || null),
    (this.predicate = n.predicate || null),
    (this.represent = n.represent || null),
    (this.representName = n.representName || null),
    (this.defaultStyle = n.defaultStyle || null),
    (this.multi = n.multi || !1),
    (this.styleAliases = Nn(n.styleAliases || null)),
    On.indexOf(this.kind) === -1)
  )
    throw new b('Unknown kind "' + this.kind + '" is specified for "' + e + '" YAML type.');
}
var S = In;
function he(e, n) {
  var i = [];
  return (
    e[n].forEach(function (l) {
      var r = i.length;
      i.forEach(function (u, o) {
        u.tag === l.tag && u.kind === l.kind && u.multi === l.multi && (r = o);
      }),
        (i[r] = l);
    }),
    i
  );
}
function Ln() {
  var e = { scalar: {}, sequence: {}, mapping: {}, fallback: {}, multi: { scalar: [], sequence: [], mapping: [], fallback: [] } },
    n,
    i;
  function l(r) {
    r.multi ? (e.multi[r.kind].push(r), e.multi.fallback.push(r)) : (e[r.kind][r.tag] = e.fallback[r.tag] = r);
  }
  for (n = 0, i = arguments.length; n < i; n += 1) arguments[n].forEach(l);
  return e;
}
function ne(e) {
  return this.extend(e);
}
ne.prototype.extend = function (n) {
  var i = [],
    l = [];
  if (n instanceof S) l.push(n);
  else if (Array.isArray(n)) l = l.concat(n);
  else if (n && (Array.isArray(n.implicit) || Array.isArray(n.explicit)))
    n.implicit && (i = i.concat(n.implicit)), n.explicit && (l = l.concat(n.explicit));
  else throw new b('Schema.extend argument should be a Type, [ Type ], or a schema definition ({ implicit: [...], explicit: [...] })');
  i.forEach(function (u) {
    if (!(u instanceof S)) throw new b('Specified list of YAML types (or a single Type object) contains a non-Type object.');
    if (u.loadKind && u.loadKind !== 'scalar')
      throw new b('There is a non-scalar type in the implicit list of a schema. Implicit resolving of such types is not supported.');
    if (u.multi) throw new b('There is a multi type in the implicit list of a schema. Multi tags can only be listed as explicit.');
  }),
    l.forEach(function (u) {
      if (!(u instanceof S)) throw new b('Specified list of YAML types (or a single Type object) contains a non-Type object.');
    });
  var r = Object.create(ne.prototype);
  return (
    (r.implicit = (this.implicit || []).concat(i)),
    (r.explicit = (this.explicit || []).concat(l)),
    (r.compiledImplicit = he(r, 'implicit')),
    (r.compiledExplicit = he(r, 'explicit')),
    (r.compiledTypeMap = Ln(r.compiledImplicit, r.compiledExplicit)),
    r
  );
};
var Oe = ne,
  Ne = new S('tag:yaml.org,2002:str', {
    kind: 'scalar',
    construct: function (e) {
      return e !== null ? e : '';
    },
  }),
  Ie = new S('tag:yaml.org,2002:seq', {
    kind: 'sequence',
    construct: function (e) {
      return e !== null ? e : [];
    },
  }),
  Le = new S('tag:yaml.org,2002:map', {
    kind: 'mapping',
    construct: function (e) {
      return e !== null ? e : {};
    },
  }),
  ke = new Oe({ explicit: [Ne, Ie, Le] });
function kn(e) {
  if (e === null) return !0;
  var n = e.length;
  return (n === 1 && e === '~') || (n === 4 && (e === 'null' || e === 'Null' || e === 'NULL'));
}
function Mn() {
  return null;
}
function Dn(e) {
  return e === null;
}
var Me = new S('tag:yaml.org,2002:null', {
  kind: 'scalar',
  resolve: kn,
  construct: Mn,
  predicate: Dn,
  represent: {
    canonical: function () {
      return '~';
    },
    lowercase: function () {
      return 'null';
    },
    uppercase: function () {
      return 'NULL';
    },
    camelcase: function () {
      return 'Null';
    },
    empty: function () {
      return '';
    },
  },
  defaultStyle: 'lowercase',
});
function Rn(e) {
  if (e === null) return !1;
  var n = e.length;
  return (n === 4 && (e === 'true' || e === 'True' || e === 'TRUE')) || (n === 5 && (e === 'false' || e === 'False' || e === 'FALSE'));
}
function Pn(e) {
  return e === 'true' || e === 'True' || e === 'TRUE';
}
function Yn(e) {
  return Object.prototype.toString.call(e) === '[object Boolean]';
}
var De = new S('tag:yaml.org,2002:bool', {
  kind: 'scalar',
  resolve: Rn,
  construct: Pn,
  predicate: Yn,
  represent: {
    lowercase: function (e) {
      return e ? 'true' : 'false';
    },
    uppercase: function (e) {
      return e ? 'TRUE' : 'FALSE';
    },
    camelcase: function (e) {
      return e ? 'True' : 'False';
    },
  },
  defaultStyle: 'lowercase',
});
function Bn(e) {
  return (48 <= e && e <= 57) || (65 <= e && e <= 70) || (97 <= e && e <= 102);
}
function Hn(e) {
  return 48 <= e && e <= 55;
}
function jn(e) {
  return 48 <= e && e <= 57;
}
function Un(e) {
  if (e === null) return !1;
  var n = e.length,
    i = 0,
    l = !1,
    r;
  if (!n) return !1;
  if (((r = e[i]), (r === '-' || r === '+') && (r = e[++i]), r === '0')) {
    if (i + 1 === n) return !0;
    if (((r = e[++i]), r === 'b')) {
      for (i++; i < n; i++)
        if (((r = e[i]), r !== '_')) {
          if (r !== '0' && r !== '1') return !1;
          l = !0;
        }
      return l && r !== '_';
    }
    if (r === 'x') {
      for (i++; i < n; i++)
        if (((r = e[i]), r !== '_')) {
          if (!Bn(e.charCodeAt(i))) return !1;
          l = !0;
        }
      return l && r !== '_';
    }
    if (r === 'o') {
      for (i++; i < n; i++)
        if (((r = e[i]), r !== '_')) {
          if (!Hn(e.charCodeAt(i))) return !1;
          l = !0;
        }
      return l && r !== '_';
    }
  }
  if (r === '_') return !1;
  for (; i < n; i++)
    if (((r = e[i]), r !== '_')) {
      if (!jn(e.charCodeAt(i))) return !1;
      l = !0;
    }
  return !(!l || r === '_');
}
function Kn(e) {
  var n = e,
    i = 1,
    l;
  if (
    (n.indexOf('_') !== -1 && (n = n.replace(/_/g, '')),
    (l = n[0]),
    (l === '-' || l === '+') && (l === '-' && (i = -1), (n = n.slice(1)), (l = n[0])),
    n === '0')
  )
    return 0;
  if (l === '0') {
    if (n[1] === 'b') return i * parseInt(n.slice(2), 2);
    if (n[1] === 'x') return i * parseInt(n.slice(2), 16);
    if (n[1] === 'o') return i * parseInt(n.slice(2), 8);
  }
  return i * parseInt(n, 10);
}
function qn(e) {
  return Object.prototype.toString.call(e) === '[object Number]' && e % 1 === 0 && !C.isNegativeZero(e);
}
var Re = new S('tag:yaml.org,2002:int', {
    kind: 'scalar',
    resolve: Un,
    construct: Kn,
    predicate: qn,
    represent: {
      binary: function (e) {
        return e >= 0 ? '0b' + e.toString(2) : '-0b' + e.toString(2).slice(1);
      },
      octal: function (e) {
        return e >= 0 ? '0o' + e.toString(8) : '-0o' + e.toString(8).slice(1);
      },
      decimal: function (e) {
        return e.toString(10);
      },
      hexadecimal: function (e) {
        return e >= 0 ? '0x' + e.toString(16).toUpperCase() : '-0x' + e.toString(16).toUpperCase().slice(1);
      },
    },
    defaultStyle: 'decimal',
    styleAliases: { binary: [2, 'bin'], octal: [8, 'oct'], decimal: [10, 'dec'], hexadecimal: [16, 'hex'] },
  }),
  Wn = new RegExp(
    '^(?:[-+]?(?:[0-9][0-9_]*)(?:\\.[0-9_]*)?(?:[eE][-+]?[0-9]+)?|\\.[0-9_]+(?:[eE][-+]?[0-9]+)?|[-+]?\\.(?:inf|Inf|INF)|\\.(?:nan|NaN|NAN))$'
  );
function Gn(e) {
  return !(e === null || !Wn.test(e) || e[e.length - 1] === '_');
}
function $n(e) {
  var n, i;
  return (
    (n = e.replace(/_/g, '').toLowerCase()),
    (i = n[0] === '-' ? -1 : 1),
    '+-'.indexOf(n[0]) >= 0 && (n = n.slice(1)),
    n === '.inf' ? (i === 1 ? Number.POSITIVE_INFINITY : Number.NEGATIVE_INFINITY) : n === '.nan' ? NaN : i * parseFloat(n, 10)
  );
}
var Xn = /^[-+]?[0-9]+e/;
function Qn(e, n) {
  var i;
  if (isNaN(e))
    switch (n) {
      case 'lowercase':
        return '.nan';
      case 'uppercase':
        return '.NAN';
      case 'camelcase':
        return '.NaN';
    }
  else if (Number.POSITIVE_INFINITY === e)
    switch (n) {
      case 'lowercase':
        return '.inf';
      case 'uppercase':
        return '.INF';
      case 'camelcase':
        return '.Inf';
    }
  else if (Number.NEGATIVE_INFINITY === e)
    switch (n) {
      case 'lowercase':
        return '-.inf';
      case 'uppercase':
        return '-.INF';
      case 'camelcase':
        return '-.Inf';
    }
  else if (C.isNegativeZero(e)) return '-0.0';
  return (i = e.toString(10)), Xn.test(i) ? i.replace('e', '.e') : i;
}
function Vn(e) {
  return Object.prototype.toString.call(e) === '[object Number]' && (e % 1 !== 0 || C.isNegativeZero(e));
}
var Pe = new S('tag:yaml.org,2002:float', { kind: 'scalar', resolve: Gn, construct: $n, predicate: Vn, represent: Qn, defaultStyle: 'lowercase' }),
  Ye = ke.extend({ implicit: [Me, De, Re, Pe] }),
  Be = Ye,
  He = new RegExp('^([0-9][0-9][0-9][0-9])-([0-9][0-9])-([0-9][0-9])$'),
  je = new RegExp(
    '^([0-9][0-9][0-9][0-9])-([0-9][0-9]?)-([0-9][0-9]?)(?:[Tt]|[ \\t]+)([0-9][0-9]?):([0-9][0-9]):([0-9][0-9])(?:\\.([0-9]*))?(?:[ \\t]*(Z|([-+])([0-9][0-9]?)(?::([0-9][0-9]))?))?$'
  );
function Zn(e) {
  return e === null ? !1 : He.exec(e) !== null || je.exec(e) !== null;
}
function zn(e) {
  var n,
    i,
    l,
    r,
    u,
    o,
    c,
    f = 0,
    a = null,
    p,
    t,
    d;
  if (((n = He.exec(e)), n === null && (n = je.exec(e)), n === null)) throw new Error('Date resolve error');
  if (((i = +n[1]), (l = +n[2] - 1), (r = +n[3]), !n[4])) return new Date(Date.UTC(i, l, r));
  if (((u = +n[4]), (o = +n[5]), (c = +n[6]), n[7])) {
    for (f = n[7].slice(0, 3); f.length < 3; ) f += '0';
    f = +f;
  }
  return (
    n[9] && ((p = +n[10]), (t = +(n[11] || 0)), (a = (p * 60 + t) * 6e4), n[9] === '-' && (a = -a)),
    (d = new Date(Date.UTC(i, l, r, u, o, c, f))),
    a && d.setTime(d.getTime() - a),
    d
  );
}
function Jn(e) {
  return e.toISOString();
}
var Ue = new S('tag:yaml.org,2002:timestamp', { kind: 'scalar', resolve: Zn, construct: zn, instanceOf: Date, represent: Jn });
function ei(e) {
  return e === '<<' || e === null;
}
var Ke = new S('tag:yaml.org,2002:merge', { kind: 'scalar', resolve: ei }),
  ue = `ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=
\r`;
function ni(e) {
  if (e === null) return !1;
  var n,
    i,
    l = 0,
    r = e.length,
    u = ue;
  for (i = 0; i < r; i++)
    if (((n = u.indexOf(e.charAt(i))), !(n > 64))) {
      if (n < 0) return !1;
      l += 6;
    }
  return l % 8 === 0;
}
function ii(e) {
  var n,
    i,
    l = e.replace(/[\r\n=]/g, ''),
    r = l.length,
    u = ue,
    o = 0,
    c = [];
  for (n = 0; n < r; n++)
    n % 4 === 0 && n && (c.push((o >> 16) & 255), c.push((o >> 8) & 255), c.push(o & 255)), (o = (o << 6) | u.indexOf(l.charAt(n)));
  return (
    (i = (r % 4) * 6),
    i === 0
      ? (c.push((o >> 16) & 255), c.push((o >> 8) & 255), c.push(o & 255))
      : i === 18
      ? (c.push((o >> 10) & 255), c.push((o >> 2) & 255))
      : i === 12 && c.push((o >> 4) & 255),
    new Uint8Array(c)
  );
}
function ri(e) {
  var n = '',
    i = 0,
    l,
    r,
    u = e.length,
    o = ue;
  for (l = 0; l < u; l++)
    l % 3 === 0 && l && ((n += o[(i >> 18) & 63]), (n += o[(i >> 12) & 63]), (n += o[(i >> 6) & 63]), (n += o[i & 63])), (i = (i << 8) + e[l]);
  return (
    (r = u % 3),
    r === 0
      ? ((n += o[(i >> 18) & 63]), (n += o[(i >> 12) & 63]), (n += o[(i >> 6) & 63]), (n += o[i & 63]))
      : r === 2
      ? ((n += o[(i >> 10) & 63]), (n += o[(i >> 4) & 63]), (n += o[(i << 2) & 63]), (n += o[64]))
      : r === 1 && ((n += o[(i >> 2) & 63]), (n += o[(i << 4) & 63]), (n += o[64]), (n += o[64])),
    n
  );
}
function li(e) {
  return Object.prototype.toString.call(e) === '[object Uint8Array]';
}
var qe = new S('tag:yaml.org,2002:binary', { kind: 'scalar', resolve: ni, construct: ii, predicate: li, represent: ri }),
  oi = Object.prototype.hasOwnProperty,
  ui = Object.prototype.toString;
function ci(e) {
  if (e === null) return !0;
  var n = [],
    i,
    l,
    r,
    u,
    o,
    c = e;
  for (i = 0, l = c.length; i < l; i += 1) {
    if (((r = c[i]), (o = !1), ui.call(r) !== '[object Object]')) return !1;
    for (u in r)
      if (oi.call(r, u))
        if (!o) o = !0;
        else return !1;
    if (!o) return !1;
    if (n.indexOf(u) === -1) n.push(u);
    else return !1;
  }
  return !0;
}
function fi(e) {
  return e !== null ? e : [];
}
var We = new S('tag:yaml.org,2002:omap', { kind: 'sequence', resolve: ci, construct: fi }),
  ai = Object.prototype.toString;
function ti(e) {
  if (e === null) return !0;
  var n,
    i,
    l,
    r,
    u,
    o = e;
  for (u = new Array(o.length), n = 0, i = o.length; n < i; n += 1) {
    if (((l = o[n]), ai.call(l) !== '[object Object]' || ((r = Object.keys(l)), r.length !== 1))) return !1;
    u[n] = [r[0], l[r[0]]];
  }
  return !0;
}
function pi(e) {
  if (e === null) return [];
  var n,
    i,
    l,
    r,
    u,
    o = e;
  for (u = new Array(o.length), n = 0, i = o.length; n < i; n += 1) (l = o[n]), (r = Object.keys(l)), (u[n] = [r[0], l[r[0]]]);
  return u;
}
var Ge = new S('tag:yaml.org,2002:pairs', { kind: 'sequence', resolve: ti, construct: pi }),
  hi = Object.prototype.hasOwnProperty;
function si(e) {
  if (e === null) return !0;
  var n,
    i = e;
  for (n in i) if (hi.call(i, n) && i[n] !== null) return !1;
  return !0;
}
function di(e) {
  return e !== null ? e : {};
}
var $e = new S('tag:yaml.org,2002:set', { kind: 'mapping', resolve: si, construct: di }),
  ce = Be.extend({ implicit: [Ue, Ke], explicit: [qe, We, Ge, $e] }),
  L = Object.prototype.hasOwnProperty,
  G = 1,
  Xe = 2,
  Qe = 3,
  $ = 4,
  ee = 1,
  mi = 2,
  se = 3,
  xi = /[\x00-\x08\x0B\x0C\x0E-\x1F\x7F-\x84\x86-\x9F\uFFFE\uFFFF]|[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?:[^\uD800-\uDBFF]|^)[\uDC00-\uDFFF]/,
  gi = /[\x85\u2028\u2029]/,
  Ai = /[,\[\]\{\}]/,
  Ve = /^(?:!|!!|![a-z\-]+!)$/i,
  Ze = /^(?:!|[^,\[\]\{\}])(?:%[0-9a-f]{2}|[0-9a-z\-#;\/\?:@&=\+\$,_\.!~\*'\(\)\[\]])*$/i;
function de(e) {
  return Object.prototype.toString.call(e);
}
function O(e) {
  return e === 10 || e === 13;
}
function k(e) {
  return e === 9 || e === 32;
}
function T(e) {
  return e === 9 || e === 32 || e === 10 || e === 13;
}
function R(e) {
  return e === 44 || e === 91 || e === 93 || e === 123 || e === 125;
}
function vi(e) {
  var n;
  return 48 <= e && e <= 57 ? e - 48 : ((n = e | 32), 97 <= n && n <= 102 ? n - 97 + 10 : -1);
}
function yi(e) {
  return e === 120 ? 2 : e === 117 ? 4 : e === 85 ? 8 : 0;
}
function wi(e) {
  return 48 <= e && e <= 57 ? e - 48 : -1;
}
function me(e) {
  return e === 48
    ? '\0'
    : e === 97
    ? '\x07'
    : e === 98
    ? '\b'
    : e === 116 || e === 9
    ? '	'
    : e === 110
    ? `
`
    : e === 118
    ? '\v'
    : e === 102
    ? '\f'
    : e === 114
    ? '\r'
    : e === 101
    ? '\x1B'
    : e === 32
    ? ' '
    : e === 34
    ? '"'
    : e === 47
    ? '/'
    : e === 92
    ? '\\'
    : e === 78
    ? ''
    : e === 95
    ? ' '
    : e === 76
    ? '\u2028'
    : e === 80
    ? '\u2029'
    : '';
}
function Ci(e) {
  return e <= 65535 ? String.fromCharCode(e) : String.fromCharCode(((e - 65536) >> 10) + 55296, ((e - 65536) & 1023) + 56320);
}
var ze = new Array(256),
  Je = new Array(256);
for (var M = 0; M < 256; M++) (ze[M] = me(M) ? 1 : 0), (Je[M] = me(M));
function _i(e, n) {
  (this.input = e),
    (this.filename = n.filename || null),
    (this.schema = n.schema || ce),
    (this.onWarning = n.onWarning || null),
    (this.legacy = n.legacy || !1),
    (this.json = n.json || !1),
    (this.listener = n.listener || null),
    (this.implicitTypes = this.schema.compiledImplicit),
    (this.typeMap = this.schema.compiledTypeMap),
    (this.length = e.length),
    (this.position = 0),
    (this.line = 0),
    (this.lineStart = 0),
    (this.lineIndent = 0),
    (this.firstTabInLine = -1),
    (this.documents = []);
}
function en(e, n) {
  var i = { name: e.filename, buffer: e.input.slice(0, -1), position: e.position, line: e.line, column: e.position - e.lineStart };
  return (i.snippet = bn(i)), new b(n, i);
}
function h(e, n) {
  throw en(e, n);
}
function X(e, n) {
  e.onWarning && e.onWarning.call(null, en(e, n));
}
var xe = {
  YAML: function (n, i, l) {
    var r, u, o;
    n.version !== null && h(n, 'duplication of %YAML directive'),
      l.length !== 1 && h(n, 'YAML directive accepts exactly one argument'),
      (r = /^([0-9]+)\.([0-9]+)$/.exec(l[0])),
      r === null && h(n, 'ill-formed argument of the YAML directive'),
      (u = parseInt(r[1], 10)),
      (o = parseInt(r[2], 10)),
      u !== 1 && h(n, 'unacceptable YAML version of the document'),
      (n.version = l[0]),
      (n.checkLineBreaks = o < 2),
      o !== 1 && o !== 2 && X(n, 'unsupported YAML version of the document');
  },
  TAG: function (n, i, l) {
    var r, u;
    l.length !== 2 && h(n, 'TAG directive accepts exactly two arguments'),
      (r = l[0]),
      (u = l[1]),
      Ve.test(r) || h(n, 'ill-formed tag handle (first argument) of the TAG directive'),
      L.call(n.tagMap, r) && h(n, 'there is a previously declared suffix for "' + r + '" tag handle'),
      Ze.test(u) || h(n, 'ill-formed tag prefix (second argument) of the TAG directive');
    try {
      u = decodeURIComponent(u);
    } catch {
      h(n, 'tag prefix is malformed: ' + u);
    }
    n.tagMap[r] = u;
  },
};
function I(e, n, i, l) {
  var r, u, o, c;
  if (n < i) {
    if (((c = e.input.slice(n, i)), l))
      for (r = 0, u = c.length; r < u; r += 1) (o = c.charCodeAt(r)), o === 9 || (32 <= o && o <= 1114111) || h(e, 'expected valid JSON character');
    else xi.test(c) && h(e, 'the stream contains non-printable characters');
    e.result += c;
  }
}
function ge(e, n, i, l) {
  var r, u, o, c;
  for (
    C.isObject(i) || h(e, 'cannot merge mappings; the provided source object is unacceptable'), r = Object.keys(i), o = 0, c = r.length;
    o < c;
    o += 1
  )
    (u = r[o]), L.call(n, u) || ((n[u] = i[u]), (l[u] = !0));
}
function P(e, n, i, l, r, u, o, c, f) {
  var a, p;
  if (Array.isArray(r))
    for (r = Array.prototype.slice.call(r), a = 0, p = r.length; a < p; a += 1)
      Array.isArray(r[a]) && h(e, 'nested arrays are not supported inside keys'),
        typeof r == 'object' && de(r[a]) === '[object Object]' && (r[a] = '[object Object]');
  if (
    (typeof r == 'object' && de(r) === '[object Object]' && (r = '[object Object]'),
    (r = String(r)),
    n === null && (n = {}),
    l === 'tag:yaml.org,2002:merge')
  )
    if (Array.isArray(u)) for (a = 0, p = u.length; a < p; a += 1) ge(e, n, u[a], i);
    else ge(e, n, u, i);
  else
    !e.json &&
      !L.call(i, r) &&
      L.call(n, r) &&
      ((e.line = o || e.line), (e.lineStart = c || e.lineStart), (e.position = f || e.position), h(e, 'duplicated mapping key')),
      r === '__proto__' ? Object.defineProperty(n, r, { configurable: !0, enumerable: !0, writable: !0, value: u }) : (n[r] = u),
      delete i[r];
  return n;
}
function fe(e) {
  var n;
  (n = e.input.charCodeAt(e.position)),
    n === 10 ? e.position++ : n === 13 ? (e.position++, e.input.charCodeAt(e.position) === 10 && e.position++) : h(e, 'a line break is expected'),
    (e.line += 1),
    (e.lineStart = e.position),
    (e.firstTabInLine = -1);
}
function w(e, n, i) {
  for (var l = 0, r = e.input.charCodeAt(e.position); r !== 0; ) {
    for (; k(r); ) r === 9 && e.firstTabInLine === -1 && (e.firstTabInLine = e.position), (r = e.input.charCodeAt(++e.position));
    if (n && r === 35)
      do r = e.input.charCodeAt(++e.position);
      while (r !== 10 && r !== 13 && r !== 0);
    if (O(r))
      for (fe(e), r = e.input.charCodeAt(e.position), l++, e.lineIndent = 0; r === 32; ) e.lineIndent++, (r = e.input.charCodeAt(++e.position));
    else break;
  }
  return i !== -1 && l !== 0 && e.lineIndent < i && X(e, 'deficient indentation'), l;
}
function Z(e) {
  var n = e.position,
    i;
  return (
    (i = e.input.charCodeAt(n)),
    !!(
      (i === 45 || i === 46) &&
      i === e.input.charCodeAt(n + 1) &&
      i === e.input.charCodeAt(n + 2) &&
      ((n += 3), (i = e.input.charCodeAt(n)), i === 0 || T(i))
    )
  );
}
function ae(e, n) {
  n === 1
    ? (e.result += ' ')
    : n > 1 &&
      (e.result += C.repeat(
        `
`,
        n - 1
      ));
}
function Ei(e, n, i) {
  var l,
    r,
    u,
    o,
    c,
    f,
    a,
    p,
    t = e.kind,
    d = e.result,
    m;
  if (
    ((m = e.input.charCodeAt(e.position)),
    T(m) ||
      R(m) ||
      m === 35 ||
      m === 38 ||
      m === 42 ||
      m === 33 ||
      m === 124 ||
      m === 62 ||
      m === 39 ||
      m === 34 ||
      m === 37 ||
      m === 64 ||
      m === 96 ||
      ((m === 63 || m === 45) && ((r = e.input.charCodeAt(e.position + 1)), T(r) || (i && R(r)))))
  )
    return !1;
  for (e.kind = 'scalar', e.result = '', u = o = e.position, c = !1; m !== 0; ) {
    if (m === 58) {
      if (((r = e.input.charCodeAt(e.position + 1)), T(r) || (i && R(r)))) break;
    } else if (m === 35) {
      if (((l = e.input.charCodeAt(e.position - 1)), T(l))) break;
    } else {
      if ((e.position === e.lineStart && Z(e)) || (i && R(m))) break;
      if (O(m))
        if (((f = e.line), (a = e.lineStart), (p = e.lineIndent), w(e, !1, -1), e.lineIndent >= n)) {
          (c = !0), (m = e.input.charCodeAt(e.position));
          continue;
        } else {
          (e.position = o), (e.line = f), (e.lineStart = a), (e.lineIndent = p);
          break;
        }
    }
    c && (I(e, u, o, !1), ae(e, e.line - f), (u = o = e.position), (c = !1)), k(m) || (o = e.position + 1), (m = e.input.charCodeAt(++e.position));
  }
  return I(e, u, o, !1), e.result ? !0 : ((e.kind = t), (e.result = d), !1);
}
function Si(e, n) {
  var i, l, r;
  if (((i = e.input.charCodeAt(e.position)), i !== 39)) return !1;
  for (e.kind = 'scalar', e.result = '', e.position++, l = r = e.position; (i = e.input.charCodeAt(e.position)) !== 0; )
    if (i === 39)
      if ((I(e, l, e.position, !0), (i = e.input.charCodeAt(++e.position)), i === 39)) (l = e.position), e.position++, (r = e.position);
      else return !0;
    else
      O(i)
        ? (I(e, l, r, !0), ae(e, w(e, !1, n)), (l = r = e.position))
        : e.position === e.lineStart && Z(e)
        ? h(e, 'unexpected end of the document within a single quoted scalar')
        : (e.position++, (r = e.position));
  h(e, 'unexpected end of the stream within a single quoted scalar');
}
function Fi(e, n) {
  var i, l, r, u, o, c;
  if (((c = e.input.charCodeAt(e.position)), c !== 34)) return !1;
  for (e.kind = 'scalar', e.result = '', e.position++, i = l = e.position; (c = e.input.charCodeAt(e.position)) !== 0; ) {
    if (c === 34) return I(e, i, e.position, !0), e.position++, !0;
    if (c === 92) {
      if ((I(e, i, e.position, !0), (c = e.input.charCodeAt(++e.position)), O(c))) w(e, !1, n);
      else if (c < 256 && ze[c]) (e.result += Je[c]), e.position++;
      else if ((o = yi(c)) > 0) {
        for (r = o, u = 0; r > 0; r--)
          (c = e.input.charCodeAt(++e.position)), (o = vi(c)) >= 0 ? (u = (u << 4) + o) : h(e, 'expected hexadecimal character');
        (e.result += Ci(u)), e.position++;
      } else h(e, 'unknown escape sequence');
      i = l = e.position;
    } else
      O(c)
        ? (I(e, i, l, !0), ae(e, w(e, !1, n)), (i = l = e.position))
        : e.position === e.lineStart && Z(e)
        ? h(e, 'unexpected end of the document within a double quoted scalar')
        : (e.position++, (l = e.position));
  }
  h(e, 'unexpected end of the stream within a double quoted scalar');
}
function bi(e, n) {
  var i = !0,
    l,
    r,
    u,
    o = e.tag,
    c,
    f = e.anchor,
    a,
    p,
    t,
    d,
    m,
    A = Object.create(null),
    v,
    y,
    x,
    s;
  if (((s = e.input.charCodeAt(e.position)), s === 91)) (p = 93), (m = !1), (c = []);
  else if (s === 123) (p = 125), (m = !0), (c = {});
  else return !1;
  for (e.anchor !== null && (e.anchorMap[e.anchor] = c), s = e.input.charCodeAt(++e.position); s !== 0; ) {
    if ((w(e, !0, n), (s = e.input.charCodeAt(e.position)), s === p))
      return e.position++, (e.tag = o), (e.anchor = f), (e.kind = m ? 'mapping' : 'sequence'), (e.result = c), !0;
    i ? s === 44 && h(e, "expected the node content, but found ','") : h(e, 'missed comma between flow collection entries'),
      (y = v = x = null),
      (t = d = !1),
      s === 63 && ((a = e.input.charCodeAt(e.position + 1)), T(a) && ((t = d = !0), e.position++, w(e, !0, n))),
      (l = e.line),
      (r = e.lineStart),
      (u = e.position),
      Y(e, n, G, !1, !0),
      (y = e.tag),
      (v = e.result),
      w(e, !0, n),
      (s = e.input.charCodeAt(e.position)),
      (d || e.line === l) && s === 58 && ((t = !0), (s = e.input.charCodeAt(++e.position)), w(e, !0, n), Y(e, n, G, !1, !0), (x = e.result)),
      m ? P(e, c, A, y, v, x, l, r, u) : t ? c.push(P(e, null, A, y, v, x, l, r, u)) : c.push(v),
      w(e, !0, n),
      (s = e.input.charCodeAt(e.position)),
      s === 44 ? ((i = !0), (s = e.input.charCodeAt(++e.position))) : (i = !1);
  }
  h(e, 'unexpected end of the stream within a flow collection');
}
function Ti(e, n) {
  var i,
    l,
    r = ee,
    u = !1,
    o = !1,
    c = n,
    f = 0,
    a = !1,
    p,
    t;
  if (((t = e.input.charCodeAt(e.position)), t === 124)) l = !1;
  else if (t === 62) l = !0;
  else return !1;
  for (e.kind = 'scalar', e.result = ''; t !== 0; )
    if (((t = e.input.charCodeAt(++e.position)), t === 43 || t === 45))
      ee === r ? (r = t === 43 ? se : mi) : h(e, 'repeat of a chomping mode identifier');
    else if ((p = wi(t)) >= 0)
      p === 0
        ? h(e, 'bad explicit indentation width of a block scalar; it cannot be less than one')
        : o
        ? h(e, 'repeat of an indentation width identifier')
        : ((c = n + p - 1), (o = !0));
    else break;
  if (k(t)) {
    do t = e.input.charCodeAt(++e.position);
    while (k(t));
    if (t === 35)
      do t = e.input.charCodeAt(++e.position);
      while (!O(t) && t !== 0);
  }
  for (; t !== 0; ) {
    for (fe(e), e.lineIndent = 0, t = e.input.charCodeAt(e.position); (!o || e.lineIndent < c) && t === 32; )
      e.lineIndent++, (t = e.input.charCodeAt(++e.position));
    if ((!o && e.lineIndent > c && (c = e.lineIndent), O(t))) {
      f++;
      continue;
    }
    if (e.lineIndent < c) {
      r === se
        ? (e.result += C.repeat(
            `
`,
            u ? 1 + f : f
          ))
        : r === ee &&
          u &&
          (e.result += `
`);
      break;
    }
    for (
      l
        ? k(t)
          ? ((a = !0),
            (e.result += C.repeat(
              `
`,
              u ? 1 + f : f
            )))
          : a
          ? ((a = !1),
            (e.result += C.repeat(
              `
`,
              f + 1
            )))
          : f === 0
          ? u && (e.result += ' ')
          : (e.result += C.repeat(
              `
`,
              f
            ))
        : (e.result += C.repeat(
            `
`,
            u ? 1 + f : f
          )),
        u = !0,
        o = !0,
        f = 0,
        i = e.position;
      !O(t) && t !== 0;

    )
      t = e.input.charCodeAt(++e.position);
    I(e, i, e.position, !1);
  }
  return !0;
}
function Ae(e, n) {
  var i,
    l = e.tag,
    r = e.anchor,
    u = [],
    o,
    c = !1,
    f;
  if (e.firstTabInLine !== -1) return !1;
  for (
    e.anchor !== null && (e.anchorMap[e.anchor] = u), f = e.input.charCodeAt(e.position);
    f !== 0 &&
    (e.firstTabInLine !== -1 && ((e.position = e.firstTabInLine), h(e, 'tab characters must not be used in indentation')),
    !(f !== 45 || ((o = e.input.charCodeAt(e.position + 1)), !T(o))));

  ) {
    if (((c = !0), e.position++, w(e, !0, -1) && e.lineIndent <= n)) {
      u.push(null), (f = e.input.charCodeAt(e.position));
      continue;
    }
    if (
      ((i = e.line),
      Y(e, n, Qe, !1, !0),
      u.push(e.result),
      w(e, !0, -1),
      (f = e.input.charCodeAt(e.position)),
      (e.line === i || e.lineIndent > n) && f !== 0)
    )
      h(e, 'bad indentation of a sequence entry');
    else if (e.lineIndent < n) break;
  }
  return c ? ((e.tag = l), (e.anchor = r), (e.kind = 'sequence'), (e.result = u), !0) : !1;
}
function Oi(e, n, i) {
  var l,
    r,
    u,
    o,
    c,
    f,
    a = e.tag,
    p = e.anchor,
    t = {},
    d = Object.create(null),
    m = null,
    A = null,
    v = null,
    y = !1,
    x = !1,
    s;
  if (e.firstTabInLine !== -1) return !1;
  for (e.anchor !== null && (e.anchorMap[e.anchor] = t), s = e.input.charCodeAt(e.position); s !== 0; ) {
    if (
      (!y && e.firstTabInLine !== -1 && ((e.position = e.firstTabInLine), h(e, 'tab characters must not be used in indentation')),
      (l = e.input.charCodeAt(e.position + 1)),
      (u = e.line),
      (s === 63 || s === 58) && T(l))
    )
      s === 63
        ? (y && (P(e, t, d, m, A, null, o, c, f), (m = A = v = null)), (x = !0), (y = !0), (r = !0))
        : y
        ? ((y = !1), (r = !0))
        : h(e, 'incomplete explicit mapping pair; a key node is missed; or followed by a non-tabulated empty line'),
        (e.position += 1),
        (s = l);
    else {
      if (((o = e.line), (c = e.lineStart), (f = e.position), !Y(e, i, Xe, !1, !0))) break;
      if (e.line === u) {
        for (s = e.input.charCodeAt(e.position); k(s); ) s = e.input.charCodeAt(++e.position);
        if (s === 58)
          (s = e.input.charCodeAt(++e.position)),
            T(s) || h(e, 'a whitespace character is expected after the key-value separator within a block mapping'),
            y && (P(e, t, d, m, A, null, o, c, f), (m = A = v = null)),
            (x = !0),
            (y = !1),
            (r = !1),
            (m = e.tag),
            (A = e.result);
        else if (x) h(e, 'can not read an implicit mapping pair; a colon is missed');
        else return (e.tag = a), (e.anchor = p), !0;
      } else if (x) h(e, 'can not read a block mapping entry; a multiline key may not be an implicit key');
      else return (e.tag = a), (e.anchor = p), !0;
    }
    if (
      ((e.line === u || e.lineIndent > n) &&
        (y && ((o = e.line), (c = e.lineStart), (f = e.position)),
        Y(e, n, $, !0, r) && (y ? (A = e.result) : (v = e.result)),
        y || (P(e, t, d, m, A, v, o, c, f), (m = A = v = null)),
        w(e, !0, -1),
        (s = e.input.charCodeAt(e.position))),
      (e.line === u || e.lineIndent > n) && s !== 0)
    )
      h(e, 'bad indentation of a mapping entry');
    else if (e.lineIndent < n) break;
  }
  return y && P(e, t, d, m, A, null, o, c, f), x && ((e.tag = a), (e.anchor = p), (e.kind = 'mapping'), (e.result = t)), x;
}
function Ni(e) {
  var n,
    i = !1,
    l = !1,
    r,
    u,
    o;
  if (((o = e.input.charCodeAt(e.position)), o !== 33)) return !1;
  if (
    (e.tag !== null && h(e, 'duplication of a tag property'),
    (o = e.input.charCodeAt(++e.position)),
    o === 60
      ? ((i = !0), (o = e.input.charCodeAt(++e.position)))
      : o === 33
      ? ((l = !0), (r = '!!'), (o = e.input.charCodeAt(++e.position)))
      : (r = '!'),
    (n = e.position),
    i)
  ) {
    do o = e.input.charCodeAt(++e.position);
    while (o !== 0 && o !== 62);
    e.position < e.length
      ? ((u = e.input.slice(n, e.position)), (o = e.input.charCodeAt(++e.position)))
      : h(e, 'unexpected end of the stream within a verbatim tag');
  } else {
    for (; o !== 0 && !T(o); )
      o === 33 &&
        (l
          ? h(e, 'tag suffix cannot contain exclamation marks')
          : ((r = e.input.slice(n - 1, e.position + 1)),
            Ve.test(r) || h(e, 'named tag handle cannot contain such characters'),
            (l = !0),
            (n = e.position + 1))),
        (o = e.input.charCodeAt(++e.position));
    (u = e.input.slice(n, e.position)), Ai.test(u) && h(e, 'tag suffix cannot contain flow indicator characters');
  }
  u && !Ze.test(u) && h(e, 'tag name cannot contain such characters: ' + u);
  try {
    u = decodeURIComponent(u);
  } catch {
    h(e, 'tag name is malformed: ' + u);
  }
  return (
    i
      ? (e.tag = u)
      : L.call(e.tagMap, r)
      ? (e.tag = e.tagMap[r] + u)
      : r === '!'
      ? (e.tag = '!' + u)
      : r === '!!'
      ? (e.tag = 'tag:yaml.org,2002:' + u)
      : h(e, 'undeclared tag handle "' + r + '"'),
    !0
  );
}
function Ii(e) {
  var n, i;
  if (((i = e.input.charCodeAt(e.position)), i !== 38)) return !1;
  for (
    e.anchor !== null && h(e, 'duplication of an anchor property'), i = e.input.charCodeAt(++e.position), n = e.position;
    i !== 0 && !T(i) && !R(i);

  )
    i = e.input.charCodeAt(++e.position);
  return e.position === n && h(e, 'name of an anchor node must contain at least one character'), (e.anchor = e.input.slice(n, e.position)), !0;
}
function Li(e) {
  var n, i, l;
  if (((l = e.input.charCodeAt(e.position)), l !== 42)) return !1;
  for (l = e.input.charCodeAt(++e.position), n = e.position; l !== 0 && !T(l) && !R(l); ) l = e.input.charCodeAt(++e.position);
  return (
    e.position === n && h(e, 'name of an alias node must contain at least one character'),
    (i = e.input.slice(n, e.position)),
    L.call(e.anchorMap, i) || h(e, 'unidentified alias "' + i + '"'),
    (e.result = e.anchorMap[i]),
    w(e, !0, -1),
    !0
  );
}
function Y(e, n, i, l, r) {
  var u,
    o,
    c,
    f = 1,
    a = !1,
    p = !1,
    t,
    d,
    m,
    A,
    v,
    y;
  if (
    (e.listener !== null && e.listener('open', e),
    (e.tag = null),
    (e.anchor = null),
    (e.kind = null),
    (e.result = null),
    (u = o = c = $ === i || Qe === i),
    l && w(e, !0, -1) && ((a = !0), e.lineIndent > n ? (f = 1) : e.lineIndent === n ? (f = 0) : e.lineIndent < n && (f = -1)),
    f === 1)
  )
    for (; Ni(e) || Ii(e); )
      w(e, !0, -1) ? ((a = !0), (c = u), e.lineIndent > n ? (f = 1) : e.lineIndent === n ? (f = 0) : e.lineIndent < n && (f = -1)) : (c = !1);
  if (
    (c && (c = a || r),
    (f === 1 || $ === i) &&
      (G === i || Xe === i ? (v = n) : (v = n + 1),
      (y = e.position - e.lineStart),
      f === 1
        ? (c && (Ae(e, y) || Oi(e, y, v))) || bi(e, v)
          ? (p = !0)
          : ((o && Ti(e, v)) || Si(e, v) || Fi(e, v)
              ? (p = !0)
              : Li(e)
              ? ((p = !0), (e.tag !== null || e.anchor !== null) && h(e, 'alias node should not have any properties'))
              : Ei(e, v, G === i) && ((p = !0), e.tag === null && (e.tag = '?')),
            e.anchor !== null && (e.anchorMap[e.anchor] = e.result))
        : f === 0 && (p = c && Ae(e, y))),
    e.tag === null)
  )
    e.anchor !== null && (e.anchorMap[e.anchor] = e.result);
  else if (e.tag === '?') {
    for (
      e.result !== null && e.kind !== 'scalar' && h(e, 'unacceptable node kind for !<?> tag; it should be "scalar", not "' + e.kind + '"'),
        t = 0,
        d = e.implicitTypes.length;
      t < d;
      t += 1
    )
      if (((A = e.implicitTypes[t]), A.resolve(e.result))) {
        (e.result = A.construct(e.result)), (e.tag = A.tag), e.anchor !== null && (e.anchorMap[e.anchor] = e.result);
        break;
      }
  } else if (e.tag !== '!') {
    if (L.call(e.typeMap[e.kind || 'fallback'], e.tag)) A = e.typeMap[e.kind || 'fallback'][e.tag];
    else
      for (A = null, m = e.typeMap.multi[e.kind || 'fallback'], t = 0, d = m.length; t < d; t += 1)
        if (e.tag.slice(0, m[t].tag.length) === m[t].tag) {
          A = m[t];
          break;
        }
    A || h(e, 'unknown tag !<' + e.tag + '>'),
      e.result !== null &&
        A.kind !== e.kind &&
        h(e, 'unacceptable node kind for !<' + e.tag + '> tag; it should be "' + A.kind + '", not "' + e.kind + '"'),
      A.resolve(e.result, e.tag)
        ? ((e.result = A.construct(e.result, e.tag)), e.anchor !== null && (e.anchorMap[e.anchor] = e.result))
        : h(e, 'cannot resolve a node with !<' + e.tag + '> explicit tag');
  }
  return e.listener !== null && e.listener('close', e), e.tag !== null || e.anchor !== null || p;
}
function ki(e) {
  var n = e.position,
    i,
    l,
    r,
    u = !1,
    o;
  for (
    e.version = null, e.checkLineBreaks = e.legacy, e.tagMap = Object.create(null), e.anchorMap = Object.create(null);
    (o = e.input.charCodeAt(e.position)) !== 0 && (w(e, !0, -1), (o = e.input.charCodeAt(e.position)), !(e.lineIndent > 0 || o !== 37));

  ) {
    for (u = !0, o = e.input.charCodeAt(++e.position), i = e.position; o !== 0 && !T(o); ) o = e.input.charCodeAt(++e.position);
    for (l = e.input.slice(i, e.position), r = [], l.length < 1 && h(e, 'directive name must not be less than one character in length'); o !== 0; ) {
      for (; k(o); ) o = e.input.charCodeAt(++e.position);
      if (o === 35) {
        do o = e.input.charCodeAt(++e.position);
        while (o !== 0 && !O(o));
        break;
      }
      if (O(o)) break;
      for (i = e.position; o !== 0 && !T(o); ) o = e.input.charCodeAt(++e.position);
      r.push(e.input.slice(i, e.position));
    }
    o !== 0 && fe(e), L.call(xe, l) ? xe[l](e, l, r) : X(e, 'unknown document directive "' + l + '"');
  }
  if (
    (w(e, !0, -1),
    e.lineIndent === 0 &&
    e.input.charCodeAt(e.position) === 45 &&
    e.input.charCodeAt(e.position + 1) === 45 &&
    e.input.charCodeAt(e.position + 2) === 45
      ? ((e.position += 3), w(e, !0, -1))
      : u && h(e, 'directives end mark is expected'),
    Y(e, e.lineIndent - 1, $, !1, !0),
    w(e, !0, -1),
    e.checkLineBreaks && gi.test(e.input.slice(n, e.position)) && X(e, 'non-ASCII line breaks are interpreted as content'),
    e.documents.push(e.result),
    e.position === e.lineStart && Z(e))
  ) {
    e.input.charCodeAt(e.position) === 46 && ((e.position += 3), w(e, !0, -1));
    return;
  }
  if (e.position < e.length - 1) h(e, 'end of the stream or a document separator is expected');
  else return;
}
function nn(e, n) {
  (e = String(e)),
    (n = n || {}),
    e.length !== 0 &&
      (e.charCodeAt(e.length - 1) !== 10 &&
        e.charCodeAt(e.length - 1) !== 13 &&
        (e += `
`),
      e.charCodeAt(0) === 65279 && (e = e.slice(1)));
  var i = new _i(e, n),
    l = e.indexOf('\0');
  for (l !== -1 && ((i.position = l), h(i, 'null byte is not allowed in input')), i.input += '\0'; i.input.charCodeAt(i.position) === 32; )
    (i.lineIndent += 1), (i.position += 1);
  for (; i.position < i.length - 1; ) ki(i);
  return i.documents;
}
function Mi(e, n, i) {
  n !== null && typeof n == 'object' && typeof i > 'u' && ((i = n), (n = null));
  var l = nn(e, i);
  if (typeof n != 'function') return l;
  for (var r = 0, u = l.length; r < u; r += 1) n(l[r]);
}
function Di(e, n) {
  var i = nn(e, n);
  if (i.length !== 0) {
    if (i.length === 1) return i[0];
    throw new b('expected a single document in the stream, but found more');
  }
}
var Ri = Mi,
  Pi = Di,
  rn = { loadAll: Ri, load: Pi },
  ln = Object.prototype.toString,
  on = Object.prototype.hasOwnProperty,
  te = 65279,
  Yi = 9,
  K = 10,
  Bi = 13,
  Hi = 32,
  ji = 33,
  Ui = 34,
  ie = 35,
  Ki = 37,
  qi = 38,
  Wi = 39,
  Gi = 42,
  un = 44,
  $i = 45,
  Q = 58,
  Xi = 61,
  Qi = 62,
  Vi = 63,
  Zi = 64,
  cn = 91,
  fn = 93,
  zi = 96,
  an = 123,
  Ji = 124,
  tn = 125,
  F = {};
F[0] = '\\0';
F[7] = '\\a';
F[8] = '\\b';
F[9] = '\\t';
F[10] = '\\n';
F[11] = '\\v';
F[12] = '\\f';
F[13] = '\\r';
F[27] = '\\e';
F[34] = '\\"';
F[92] = '\\\\';
F[133] = '\\N';
F[160] = '\\_';
F[8232] = '\\L';
F[8233] = '\\P';
var er = ['y', 'Y', 'yes', 'Yes', 'YES', 'on', 'On', 'ON', 'n', 'N', 'no', 'No', 'NO', 'off', 'Off', 'OFF'],
  nr = /^[-+]?[0-9_]+(?::[0-9_]+)+(?:\.[0-9_]*)?$/;
function ir(e, n) {
  var i, l, r, u, o, c, f;
  if (n === null) return {};
  for (i = {}, l = Object.keys(n), r = 0, u = l.length; r < u; r += 1)
    (o = l[r]),
      (c = String(n[o])),
      o.slice(0, 2) === '!!' && (o = 'tag:yaml.org,2002:' + o.slice(2)),
      (f = e.compiledTypeMap.fallback[o]),
      f && on.call(f.styleAliases, c) && (c = f.styleAliases[c]),
      (i[o] = c);
  return i;
}
function rr(e) {
  var n, i, l;
  if (((n = e.toString(16).toUpperCase()), e <= 255)) (i = 'x'), (l = 2);
  else if (e <= 65535) (i = 'u'), (l = 4);
  else if (e <= 4294967295) (i = 'U'), (l = 8);
  else throw new b('code point within a string may not be greater than 0xFFFFFFFF');
  return '\\' + i + C.repeat('0', l - n.length) + n;
}
var lr = 1,
  q = 2;
function or(e) {
  (this.schema = e.schema || ce),
    (this.indent = Math.max(1, e.indent || 2)),
    (this.noArrayIndent = e.noArrayIndent || !1),
    (this.skipInvalid = e.skipInvalid || !1),
    (this.flowLevel = C.isNothing(e.flowLevel) ? -1 : e.flowLevel),
    (this.styleMap = ir(this.schema, e.styles || null)),
    (this.sortKeys = e.sortKeys || !1),
    (this.lineWidth = e.lineWidth || 80),
    (this.noRefs = e.noRefs || !1),
    (this.noCompatMode = e.noCompatMode || !1),
    (this.condenseFlow = e.condenseFlow || !1),
    (this.quotingType = e.quotingType === '"' ? q : lr),
    (this.forceQuotes = e.forceQuotes || !1),
    (this.replacer = typeof e.replacer == 'function' ? e.replacer : null),
    (this.implicitTypes = this.schema.compiledImplicit),
    (this.explicitTypes = this.schema.compiledExplicit),
    (this.tag = null),
    (this.result = ''),
    (this.duplicates = []),
    (this.usedDuplicates = null);
}
function ve(e, n) {
  for (var i = C.repeat(' ', n), l = 0, r = -1, u = '', o, c = e.length; l < c; )
    (r = e.indexOf(
      `
`,
      l
    )),
      r === -1 ? ((o = e.slice(l)), (l = c)) : ((o = e.slice(l, r + 1)), (l = r + 1)),
      o.length &&
        o !==
          `
` &&
        (u += i),
      (u += o);
  return u;
}
function re(e, n) {
  return (
    `
` + C.repeat(' ', e.indent * n)
  );
}
function ur(e, n) {
  var i, l, r;
  for (i = 0, l = e.implicitTypes.length; i < l; i += 1) if (((r = e.implicitTypes[i]), r.resolve(n))) return !0;
  return !1;
}
function V(e) {
  return e === Hi || e === Yi;
}
function W(e) {
  return (
    (32 <= e && e <= 126) ||
    (161 <= e && e <= 55295 && e !== 8232 && e !== 8233) ||
    (57344 <= e && e <= 65533 && e !== te) ||
    (65536 <= e && e <= 1114111)
  );
}
function ye(e) {
  return W(e) && e !== te && e !== Bi && e !== K;
}
function we(e, n, i) {
  var l = ye(e),
    r = l && !V(e);
  return (
    ((i ? l : l && e !== un && e !== cn && e !== fn && e !== an && e !== tn) && e !== ie && !(n === Q && !r)) ||
    (ye(n) && !V(n) && e === ie) ||
    (n === Q && r)
  );
}
function cr(e) {
  return (
    W(e) &&
    e !== te &&
    !V(e) &&
    e !== $i &&
    e !== Vi &&
    e !== Q &&
    e !== un &&
    e !== cn &&
    e !== fn &&
    e !== an &&
    e !== tn &&
    e !== ie &&
    e !== qi &&
    e !== Gi &&
    e !== ji &&
    e !== Ji &&
    e !== Xi &&
    e !== Qi &&
    e !== Wi &&
    e !== Ui &&
    e !== Ki &&
    e !== Zi &&
    e !== zi
  );
}
function fr(e) {
  return !V(e) && e !== Q;
}
function j(e, n) {
  var i = e.charCodeAt(n),
    l;
  return i >= 55296 && i <= 56319 && n + 1 < e.length && ((l = e.charCodeAt(n + 1)), l >= 56320 && l <= 57343)
    ? (i - 55296) * 1024 + l - 56320 + 65536
    : i;
}
function pn(e) {
  var n = /^\n* /;
  return n.test(e);
}
var hn = 1,
  le = 2,
  sn = 3,
  dn = 4,
  D = 5;
function ar(e, n, i, l, r, u, o, c) {
  var f,
    a = 0,
    p = null,
    t = !1,
    d = !1,
    m = l !== -1,
    A = -1,
    v = cr(j(e, 0)) && fr(j(e, e.length - 1));
  if (n || o)
    for (f = 0; f < e.length; a >= 65536 ? (f += 2) : f++) {
      if (((a = j(e, f)), !W(a))) return D;
      (v = v && we(a, p, c)), (p = a);
    }
  else {
    for (f = 0; f < e.length; a >= 65536 ? (f += 2) : f++) {
      if (((a = j(e, f)), a === K)) (t = !0), m && ((d = d || (f - A - 1 > l && e[A + 1] !== ' ')), (A = f));
      else if (!W(a)) return D;
      (v = v && we(a, p, c)), (p = a);
    }
    d = d || (m && f - A - 1 > l && e[A + 1] !== ' ');
  }
  return !t && !d ? (v && !o && !r(e) ? hn : u === q ? D : le) : i > 9 && pn(e) ? D : o ? (u === q ? D : le) : d ? dn : sn;
}
function tr(e, n, i, l, r) {
  e.dump = (function () {
    if (n.length === 0) return e.quotingType === q ? '""' : "''";
    if (!e.noCompatMode && (er.indexOf(n) !== -1 || nr.test(n))) return e.quotingType === q ? '"' + n + '"' : "'" + n + "'";
    var u = e.indent * Math.max(1, i),
      o = e.lineWidth === -1 ? -1 : Math.max(Math.min(e.lineWidth, 40), e.lineWidth - u),
      c = l || (e.flowLevel > -1 && i >= e.flowLevel);
    function f(a) {
      return ur(e, a);
    }
    switch (ar(n, c, e.indent, o, f, e.quotingType, e.forceQuotes && !l, r)) {
      case hn:
        return n;
      case le:
        return "'" + n.replace(/'/g, "''") + "'";
      case sn:
        return '|' + Ce(n, e.indent) + _e(ve(n, u));
      case dn:
        return '>' + Ce(n, e.indent) + _e(ve(pr(n, o), u));
      case D:
        return '"' + hr(n) + '"';
      default:
        throw new b('impossible error: invalid scalar style');
    }
  })();
}
function Ce(e, n) {
  var i = pn(e) ? String(n) : '',
    l =
      e[e.length - 1] ===
      `
`,
    r =
      l &&
      (e[e.length - 2] ===
        `
` ||
        e ===
          `
`),
    u = r ? '+' : l ? '' : '-';
  return (
    i +
    u +
    `
`
  );
}
function _e(e) {
  return e[e.length - 1] ===
    `
`
    ? e.slice(0, -1)
    : e;
}
function pr(e, n) {
  for (
    var i = /(\n+)([^\n]*)/g,
      l = (function () {
        var a = e.indexOf(`
`);
        return (a = a !== -1 ? a : e.length), (i.lastIndex = a), Ee(e.slice(0, a), n);
      })(),
      r =
        e[0] ===
          `
` || e[0] === ' ',
      u,
      o;
    (o = i.exec(e));

  ) {
    var c = o[1],
      f = o[2];
    (u = f[0] === ' '),
      (l +=
        c +
        (!r && !u && f !== ''
          ? `
`
          : '') +
        Ee(f, n)),
      (r = u);
  }
  return l;
}
function Ee(e, n) {
  if (e === '' || e[0] === ' ') return e;
  for (var i = / [^ ]/g, l, r = 0, u, o = 0, c = 0, f = ''; (l = i.exec(e)); )
    (c = l.index),
      c - r > n &&
        ((u = o > r ? o : c),
        (f +=
          `
` + e.slice(r, u)),
        (r = u + 1)),
      (o = c);
  return (
    (f += `
`),
    e.length - r > n && o > r
      ? (f +=
          e.slice(r, o) +
          `
` +
          e.slice(o + 1))
      : (f += e.slice(r)),
    f.slice(1)
  );
}
function hr(e) {
  for (var n = '', i = 0, l, r = 0; r < e.length; i >= 65536 ? (r += 2) : r++)
    (i = j(e, r)), (l = F[i]), !l && W(i) ? ((n += e[r]), i >= 65536 && (n += e[r + 1])) : (n += l || rr(i));
  return n;
}
function sr(e, n, i) {
  var l = '',
    r = e.tag,
    u,
    o,
    c;
  for (u = 0, o = i.length; u < o; u += 1)
    (c = i[u]),
      e.replacer && (c = e.replacer.call(i, String(u), c)),
      (N(e, n, c, !1, !1) || (typeof c > 'u' && N(e, n, null, !1, !1))) && (l !== '' && (l += ',' + (e.condenseFlow ? '' : ' ')), (l += e.dump));
  (e.tag = r), (e.dump = '[' + l + ']');
}
function Se(e, n, i, l) {
  var r = '',
    u = e.tag,
    o,
    c,
    f;
  for (o = 0, c = i.length; o < c; o += 1)
    (f = i[o]),
      e.replacer && (f = e.replacer.call(i, String(o), f)),
      (N(e, n + 1, f, !0, !0, !1, !0) || (typeof f > 'u' && N(e, n + 1, null, !0, !0, !1, !0))) &&
        ((!l || r !== '') && (r += re(e, n)), e.dump && K === e.dump.charCodeAt(0) ? (r += '-') : (r += '- '), (r += e.dump));
  (e.tag = u), (e.dump = r || '[]');
}
function dr(e, n, i) {
  var l = '',
    r = e.tag,
    u = Object.keys(i),
    o,
    c,
    f,
    a,
    p;
  for (o = 0, c = u.length; o < c; o += 1)
    (p = ''),
      l !== '' && (p += ', '),
      e.condenseFlow && (p += '"'),
      (f = u[o]),
      (a = i[f]),
      e.replacer && (a = e.replacer.call(i, f, a)),
      N(e, n, f, !1, !1) &&
        (e.dump.length > 1024 && (p += '? '),
        (p += e.dump + (e.condenseFlow ? '"' : '') + ':' + (e.condenseFlow ? '' : ' ')),
        N(e, n, a, !1, !1) && ((p += e.dump), (l += p)));
  (e.tag = r), (e.dump = '{' + l + '}');
}
function mr(e, n, i, l) {
  var r = '',
    u = e.tag,
    o = Object.keys(i),
    c,
    f,
    a,
    p,
    t,
    d;
  if (e.sortKeys === !0) o.sort();
  else if (typeof e.sortKeys == 'function') o.sort(e.sortKeys);
  else if (e.sortKeys) throw new b('sortKeys must be a boolean or a function');
  for (c = 0, f = o.length; c < f; c += 1)
    (d = ''),
      (!l || r !== '') && (d += re(e, n)),
      (a = o[c]),
      (p = i[a]),
      e.replacer && (p = e.replacer.call(i, a, p)),
      N(e, n + 1, a, !0, !0, !0) &&
        ((t = (e.tag !== null && e.tag !== '?') || (e.dump && e.dump.length > 1024)),
        t && (e.dump && K === e.dump.charCodeAt(0) ? (d += '?') : (d += '? ')),
        (d += e.dump),
        t && (d += re(e, n)),
        N(e, n + 1, p, !0, t) && (e.dump && K === e.dump.charCodeAt(0) ? (d += ':') : (d += ': '), (d += e.dump), (r += d)));
  (e.tag = u), (e.dump = r || '{}');
}
function Fe(e, n, i) {
  var l, r, u, o, c, f;
  for (r = i ? e.explicitTypes : e.implicitTypes, u = 0, o = r.length; u < o; u += 1)
    if (
      ((c = r[u]),
      (c.instanceOf || c.predicate) && (!c.instanceOf || (typeof n == 'object' && n instanceof c.instanceOf)) && (!c.predicate || c.predicate(n)))
    ) {
      if ((i ? (c.multi && c.representName ? (e.tag = c.representName(n)) : (e.tag = c.tag)) : (e.tag = '?'), c.represent)) {
        if (((f = e.styleMap[c.tag] || c.defaultStyle), ln.call(c.represent) === '[object Function]')) l = c.represent(n, f);
        else if (on.call(c.represent, f)) l = c.represent[f](n, f);
        else throw new b('!<' + c.tag + '> tag resolver accepts not "' + f + '" style');
        e.dump = l;
      }
      return !0;
    }
  return !1;
}
function N(e, n, i, l, r, u, o) {
  (e.tag = null), (e.dump = i), Fe(e, i, !1) || Fe(e, i, !0);
  var c = ln.call(e.dump),
    f = l,
    a;
  l && (l = e.flowLevel < 0 || e.flowLevel > n);
  var p = c === '[object Object]' || c === '[object Array]',
    t,
    d;
  if (
    (p && ((t = e.duplicates.indexOf(i)), (d = t !== -1)),
    ((e.tag !== null && e.tag !== '?') || d || (e.indent !== 2 && n > 0)) && (r = !1),
    d && e.usedDuplicates[t])
  )
    e.dump = '*ref_' + t;
  else {
    if ((p && d && !e.usedDuplicates[t] && (e.usedDuplicates[t] = !0), c === '[object Object]'))
      l && Object.keys(e.dump).length !== 0
        ? (mr(e, n, e.dump, r), d && (e.dump = '&ref_' + t + e.dump))
        : (dr(e, n, e.dump), d && (e.dump = '&ref_' + t + ' ' + e.dump));
    else if (c === '[object Array]')
      l && e.dump.length !== 0
        ? (e.noArrayIndent && !o && n > 0 ? Se(e, n - 1, e.dump, r) : Se(e, n, e.dump, r), d && (e.dump = '&ref_' + t + e.dump))
        : (sr(e, n, e.dump), d && (e.dump = '&ref_' + t + ' ' + e.dump));
    else if (c === '[object String]') e.tag !== '?' && tr(e, e.dump, n, u, f);
    else {
      if (c === '[object Undefined]') return !1;
      if (e.skipInvalid) return !1;
      throw new b('unacceptable kind of an object to dump ' + c);
    }
    e.tag !== null &&
      e.tag !== '?' &&
      ((a = encodeURI(e.tag[0] === '!' ? e.tag.slice(1) : e.tag).replace(/!/g, '%21')),
      e.tag[0] === '!' ? (a = '!' + a) : a.slice(0, 18) === 'tag:yaml.org,2002:' ? (a = '!!' + a.slice(18)) : (a = '!<' + a + '>'),
      (e.dump = a + ' ' + e.dump));
  }
  return !0;
}
function xr(e, n) {
  var i = [],
    l = [],
    r,
    u;
  for (oe(e, i, l), r = 0, u = l.length; r < u; r += 1) n.duplicates.push(i[l[r]]);
  n.usedDuplicates = new Array(u);
}
function oe(e, n, i) {
  var l, r, u;
  if (e !== null && typeof e == 'object')
    if (((r = n.indexOf(e)), r !== -1)) i.indexOf(r) === -1 && i.push(r);
    else if ((n.push(e), Array.isArray(e))) for (r = 0, u = e.length; r < u; r += 1) oe(e[r], n, i);
    else for (l = Object.keys(e), r = 0, u = l.length; r < u; r += 1) oe(e[l[r]], n, i);
}
function gr(e, n) {
  n = n || {};
  var i = new or(n);
  i.noRefs || xr(e, i);
  var l = e;
  return (
    i.replacer && (l = i.replacer.call({ '': l }, '', l)),
    N(i, 0, l, !0, !0)
      ? i.dump +
        `
`
      : ''
  );
}
var Ar = gr,
  vr = { dump: Ar };
function pe(e, n) {
  return function () {
    throw new Error('Function yaml.' + e + ' is removed in js-yaml 4. Use yaml.' + n + ' instead, which is now safe by default.');
  };
}
var yr = S,
  wr = Oe,
  Cr = ke,
  _r = Ye,
  Er = Be,
  Sr = ce,
  Fr = rn.load,
  br = rn.loadAll,
  Tr = vr.dump,
  Or = b,
  Nr = { binary: qe, float: Pe, map: Le, null: Me, pairs: Ge, set: $e, timestamp: Ue, bool: De, int: Re, merge: Ke, omap: We, seq: Ie, str: Ne },
  Ir = pe('safeLoad', 'load'),
  Lr = pe('safeLoadAll', 'loadAll'),
  kr = pe('safeDump', 'dump'),
  Mr = {
    Type: yr,
    Schema: wr,
    FAILSAFE_SCHEMA: Cr,
    JSON_SCHEMA: _r,
    CORE_SCHEMA: Er,
    DEFAULT_SCHEMA: Sr,
    load: Fr,
    loadAll: br,
    dump: Tr,
    YAMLException: Or,
    types: Nr,
    safeLoad: Ir,
    safeLoadAll: Lr,
    safeDump: kr,
  };
(async () => {
  let e = 0;
  const n = [],
    i = [];
  let l = !0;
  const r = (x) => new Promise((s) => setTimeout(s, x)),
    u = window.sitemapCrawlerSleepInterval,
    o = typeof u == 'number' && u >= 0 ? u : 2e3,
    c = () => {
      try {
        return window.self === window.top;
      } catch {
        return !1;
      }
    };
  if (!('showSaveFilePicker' in window)) {
    console.log('Browser is not supported, please use Chrome or Edge.');
    return;
  }
  if (!c()) {
    console.log('The script only work for top window.');
    return;
  }
  const a = await (async () => {
      const s = location.hostname.replaceAll('.', '-') + '.tests.yaml';
      try {
        const g = { suggestedName: s, types: [{ description: 'Test File', accept: { 'text/yaml': ['.tests.yaml'] } }] };
        return await (await window.showSaveFilePicker(g)).createWritable();
      } catch (g) {
        if (g.name !== 'AbortError') {
          console.error(g.name, g.message);
          return;
        }
      }
    })(),
    p = (x) => {
      if (x.nodeType === Node.TEXT_NODE) return;
      if (x.nodeType !== Node.ELEMENT_NODE) {
        console.log(x);
        return;
      }
      const s = x.querySelectorAll('url > loc');
      [].forEach.call(s, (g) => {
        const _ = g.textContent;
        if ((i.push(_), location.origin.toLowerCase() == 'https://tuyen.blog')) {
          const E = new URL(_);
          E.searchParams.set('theme', 'dark'), i.push(E.href), E.searchParams.set('night', 'true'), i.push(E.href);
        }
      }),
        i.length > 1e3 && A();
    },
    t = async (x) => {
      if (x.nodeType === Node.TEXT_NODE) return;
      if (x.nodeType !== Node.ELEMENT_NODE) {
        console.log(x);
        return;
      }
      const s = x.querySelector('loc'),
        g = s == null ? void 0 : s.textContent;
      if (g && !n.includes(g)) {
        n.push(g), console.log(`Fetching: ${g} - ${e} URLs found.`);
        const _ = await fetch(g)
          .then((E) => E.text())
          .catch((E) => {
            console.log(`Cannot fetch ${g}`, E);
          });
        if (_)
          try {
            const B = new DOMParser().parseFromString(_, 'application/xml');
            for (let H = 0; H < B.childNodes.length; H++) await d(B.childNodes[H]);
          } catch (E) {
            console.log('Cannot parse XML for ' + g, E);
          }
      }
    },
    d = async (x) => {
      const s = x.nodeName.toLowerCase();
      switch (x.nodeType) {
        case Node.ELEMENT_NODE:
          switch (s) {
            case 'sitemapindex':
              for (let g = 0; g < x.childNodes.length; g++) await t(x.childNodes[g]), o > 0 && (await r(o));
              break;
            case 'urlset':
              for (let g = 0; g < x.childNodes.length; g++) p(x.childNodes[g]);
              break;
            default:
              console.log('Unknow node name:', s);
          }
          break;
      }
    },
    m = async (x, s) => {
      try {
        const _ = new DOMParser().parseFromString(s, 'application/xml');
        for (let E = 0; E < _.childNodes.length; E++) await d(_.childNodes[E]);
      } catch (g) {
        console.log('Cannot parse XML for ' + x, g);
      }
    },
    A = () => {
      i.sort();
      const x = [];
      [].forEach.call(i, (_) => {
        x.push({ url: _ });
      });
      const s = { scenarios: x },
        g = Mr.dump(s, { lineWidth: -1, noCompatMode: !0 });
      if (l) a == null || a.write(g);
      else {
        const _ = g.split(`
`);
        _.splice(0, 1),
          a == null ||
            a.write(
              _.join(`
`)
            );
      }
      (l = !1), (e += i.length), (i.length = 0);
    },
    v = async (x) => {
      const s = await fetch(x)
        .then((g) => g.text())
        .catch((g) => (console.log(`Cannot fetch ${x}.`, g), null));
      if (s)
        try {
          await m(x, s);
        } catch (g) {
          console.log('Cannot parse XML for ' + x, g);
        }
    },
    y = async () => {
      let x = 0;
      console.log('Fetching: /robots.txt');
      const s = await fetch('/robots.txt')
        .then((g) => g.text())
        .catch((g) => (console.log('Cannot read robots.txt.', g), null));
      if (s) {
        const g = s.split(/\r*\n/gi);
        for (let _ = 0; _ < g.length; _++) {
          const E = g[_],
            B = /^sitemap:\s+(.+?)\s*$/gi.exec(E);
          if (!B) continue;
          x++;
          const H = B[1];
          await v(H);
        }
      }
      x === 0 && (console.log('Fetching: /sitemap.xml'), await v('/sitemap.xml'));
    };
  if (a)
    try {
      await y(), A();
    } catch (x) {
      console.log(x);
    } finally {
      await a.close();
    }
  console.log(`FINISH! - ${e} URLs found.`);
})();
