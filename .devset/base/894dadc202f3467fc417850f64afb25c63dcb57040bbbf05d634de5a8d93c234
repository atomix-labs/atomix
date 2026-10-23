"""The crates a release publishes to crates.io: every package of the workspace whose `publish` allows
crates.io.

Usage: cargo-publish.py metadata [--book <url>] | check | publish

`metadata` holds each to what its page on crates.io shows, and says what to add where one falls
short: a README, every link and image in it absolute; keywords and categories crates.io takes; the
licence files `license` names, inside the package; and, with `--book`, the book's URL as `homepage`
or `documentation`. `check` packages each, and builds it from its package, as crates.io will.
Between releases, while crates.io has a crate at its version already, cargo would build its
dependents against crates.io's copy, not the working tree's, so each is packaged without the build;
a release's check, its versions new, builds them all, from what this check packaged and not what an
earlier one did. `publish` publishes each that crates.io does not have at its version, dependencies
first, so a release that stopped halfway publishes the rest when it runs again.
"""

import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import urllib.error
import urllib.request

API = "https://crates.io/api/v1/crates"
# crates.io refuses a request without one that names who is asking.
USER_AGENT = "atxp crates-io (https://github.com/atomix-labs/atxp)"

# What crates.io takes of a crate's keywords and categories, as its publish endpoint holds them.
MOST_KEYWORDS = 5
MOST_CATEGORIES = 5
KEYWORD = re.compile(r"[A-Za-z0-9][A-Za-z0-9_+-]{0,19}")
CATEGORIES = pathlib.Path(__file__).with_name("cargo-publish") / "categories.txt"

# A README's links and images, read from its HTML and Markdown alike.
FENCE = re.compile(r"^ {0,3}(`{3,}|~{3,})")
SPAN = re.compile(r"(`+).+?\1")
ATTRIBUTE = re.compile(r"""\b(src|srcset|href)\s*=\s*(["'])(.*?)\2""", re.IGNORECASE)
LINK = re.compile(r"\]\(\s*<?([^)\s>]+)")
IMAGE = re.compile(r"!\[[^\]]*\]\(\s*<?([^)\s>]+)")
DEFINITION = re.compile(r"^ {0,3}\[[^\]]+\]:\s*<?([^\s>]+)")
# A scheme, a fragment of this page, or a host: a link that resolves wherever the README is shown.
ABSOLUTE = re.compile(r"[A-Za-z][A-Za-z0-9+.-]*:|#|//")
BLOCK = ("<!-- >>> devset: project >>> -->", "<!-- <<< devset: project <<< -->")


def packages():
    """The workspace's root, and its packages crates.io takes, in the workspace's order."""
    out = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"],
        capture_output=True,
        text=True,
        check=True,
    )
    metadata = json.loads(out.stdout)
    # `publish` is null for any registry, or the registries allowed: `[]` is `publish = false`.
    allowed = [
        package for package in metadata["packages"] if package["publish"] is None or "crates-io" in package["publish"]
    ]
    return pathlib.Path(metadata["workspace_root"]), allowed


def publishable():
    """The workspace's packages crates.io takes, as (name, version), in the workspace's order."""
    return [(package["name"], package["version"]) for package in packages()[1]]


def published(name, version):
    """Whether crates.io has `name` at `version`."""
    request = urllib.request.Request(f"{API}/{name}/{version}", headers={"User-Agent": USER_AGENT})
    try:
        with urllib.request.urlopen(request, timeout=60):
            return True
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return False
        raise


def packaged(name):
    """The files cargo puts in `name`'s package, as paths inside it."""
    out = subprocess.run(
        ["cargo", "package", "--list", "--allow-dirty", "--locked", "--package", name],
        stdout=subprocess.PIPE,
        text=True,
        check=True,
    )
    return set(out.stdout.split())


def categories():
    """Every category slug crates.io takes."""
    lines = (line.strip() for line in CATEGORIES.read_text().splitlines())
    return {line for line in lines if line and not line.startswith("#")}


def licence_files(expression):
    """For each licence `expression` names, the files any one of which holds its text, as the project
    profile writes them: `LICENSE` for a single licence, and `LICENSE-<NAME>` for each of several,
    `LICENSE-MIT` and `LICENSE-APACHE` for `MIT OR Apache-2.0`."""
    expression = re.sub(r"\bWITH\s+\S+", "", expression)
    ids = [token.rstrip("+") for token in re.findall(r"[A-Za-z0-9.+-]+", expression)]
    ids = [token for token in ids if token not in {"OR", "AND"}]
    wanted = []
    for licence in ids:
        names = {f"LICENSE-{re.sub(r'-[0-9].*$', '', licence).upper()}", f"LICENSE-{licence.upper()}"}
        if len(ids) == 1:
            names.add("LICENSE")
        wanted.append(sorted(names, key=len))
    return wanted


def check_readme(package, root, files):
    """The crate's README, which crates.io shows: one, packaged."""
    readme = package["readme"]
    if readme is None:
        return [
            'readme: none; add a `README.md` beside this file, or `readme = "../../README.md"` '
            "for the crate the root README is for"
        ]
    path = pathlib.Path(package["manifest_path"]).parent / readme
    if not path.is_file():
        return [f"readme: `{readme}` names no file"]
    if path.name not in files and readme not in files:
        return [f"readme: `{readme}` is not in the package; take it out of `exclude`"]
    return []


def check_keywords(package):
    """The crate's keywords, which crates.io's search takes: one to five, each a word it allows."""
    keywords = package["keywords"]
    if not keywords:
        return ['keywords: none; add up to five, the words a search on crates.io finds it by, `keywords = ["…"]`']
    found = []
    if len(keywords) > MOST_KEYWORDS:
        found.append(f"keywords: {len(keywords)}, and crates.io takes at most {MOST_KEYWORDS}")
    found += [
        f"keywords: `{keyword}` is no keyword crates.io takes: at most 20 ASCII letters, digits, `_`, "
        "`-` or `+`, the first a letter or a digit"
        for keyword in keywords
        if not KEYWORD.fullmatch(keyword)
    ]
    return found


def check_categories(package, slugs):
    """The crate's categories, which crates.io lists it under: one to five, each a slug it has."""
    chosen = package["categories"]
    if not chosen:
        return [
            "categories: none; add up to five of crates.io's slugs, https://crates.io/category_slugs, "
            '`categories = ["…"]`'
        ]
    found = []
    if len(chosen) > MOST_CATEGORIES:
        found.append(f"categories: {len(chosen)}, and crates.io takes at most {MOST_CATEGORIES}")
    found += [
        f"categories: `{slug}` is no category of crates.io's; take a slug from https://crates.io/category_slugs"
        for slug in chosen
        if slug not in slugs
    ]
    return found


def check_book(package, book):
    """The book's URL, as the link crates.io shows beside the crate's docs."""
    if not book or package["homepage"] or package["documentation"]:
        return []
    return [
        f'homepage: none; set `homepage = "{book}"`, the book, in `[workspace.package]`, and '
        "`homepage.workspace = true` here"
    ]


def check_licences(package, root, files):
    """The text of each licence `license` names, in the package, as each licence asks."""
    if not package["license"]:
        return []
    crate = pathlib.Path(package["manifest_path"]).parent
    found = []
    for names in licence_files(package["license"]):
        if any(name in files or any(path.startswith(f"{name}.") for path in files) for name in names):
            continue
        name = next((name for name in names if (root / name).is_file()), names[0])
        if (root / name).is_file() and crate != root:
            target, link = os.path.relpath(root / name, crate), (crate / name).relative_to(root)
            found.append(f"license: `{name}` is not in the package; link it, `ln -s {target} {link}`")
        else:
            found.append(f"license: `{name}` is not in the package; add it beside this file")
    return found


def repository_urls(package):
    """The URLs a path of the repository has on GitHub, a page's and a file's, or None elsewhere."""
    match = re.fullmatch(r"https://github\.com/([^/]+)/([^/]+?)(?:\.git)?/?", package["repository"] or "")
    if not match:
        return None
    owner, name = match.groups()
    return f"https://github.com/{owner}/{name}/blob/main/", f"https://raw.githubusercontent.com/{owner}/{name}/main/"


def links(text):
    """Each link and image of a Markdown page, as (line number, target, whether an image, whether in
    the project profile's block); a target in code is none."""
    inside, block = None, False
    for lineno, line in enumerate(text.splitlines(), 1):
        fence = FENCE.match(line)
        if fence and (inside is None or (fence.group(1)[0] == inside[0] and len(fence.group(1)) >= len(inside))):
            inside = None if inside else fence.group(1)
            continue
        if inside:
            continue
        block = (block or line.strip() == BLOCK[0]) and line.strip() != BLOCK[1]
        line = SPAN.sub("", line)
        for attribute in ATTRIBUTE.finditer(line):
            kind, value = attribute.group(1).lower(), attribute.group(3)
            targets = [part.split()[0] for part in value.split(",") if part.split()] if kind == "srcset" else [value]
            yield from ((lineno, target, kind != "href", block) for target in targets)
        # A badge is an image inside a link, `[![CI](<image>)](<page>)`: two targets.
        images = {image.start(1) for image in IMAGE.finditer(line)}
        for link in LINK.finditer(line):
            yield lineno, link.group(1), link.start(1) in images, block
        definition = DEFINITION.match(line)
        if definition:
            yield lineno, definition.group(1), False, block


def check_links(package, root, seen):
    """Each relative link or image in the crate's README, which crates.io shows away from the
    repository, as (README, line, message); a README two crates share is read once."""
    readme = package["readme"] and pathlib.Path(package["manifest_path"]).parent / package["readme"]
    if not readme or not readme.is_file() or readme.resolve() in seen:
        return []
    seen.add(readme.resolve())
    urls = repository_urls(package)
    found = []
    for lineno, target, image, block in links(readme.read_text()):
        if not target or ABSOLUTE.match(target):
            continue
        path = os.path.normpath(readme.parent / re.split(r"[#?]", target)[0])
        inside = pathlib.Path(path).resolve().is_relative_to(root.resolve())
        relative = pathlib.Path(path).resolve().relative_to(root.resolve()).as_posix() if inside else target
        url = f"`{urls[1 if image else 0]}{relative}`" if urls and inside else "an absolute URL"
        if block and urls and inside and re.search(r"-(light|dark)\.svg$", relative):
            logo = re.sub(r"-(light|dark)\.svg$", "", relative)
            advice = f"answer `logo` with a URL, `devset apply --var logo={urls[1]}{logo}`"
        else:
            advice = f"write {url}"
        found.append(
            (
                readme.resolve().relative_to(root.resolve()),
                lineno,
                f"link: `{target}` is relative, and crates.io shows this README; {advice}",
            )
        )
    return found


def metadata(book):
    """Holds every publishable crate to what its page on crates.io shows, and names what each lacks."""
    root, crates = packages()
    if not crates:
        print("crates-io: the workspace publishes no crate")
        return 0
    slugs, seen, found = categories(), set(), []
    for package in crates:
        manifest = pathlib.Path(package["manifest_path"]).relative_to(root)
        files = packaged(package["name"])
        messages = [
            *check_readme(package, root, files),
            *check_keywords(package),
            *check_categories(package, slugs),
            *check_book(package, book),
            *check_licences(package, root, files),
        ]
        found += [(manifest, None, message) for message in messages]
        found += check_links(package, root, seen)
    for path, lineno, message in found:
        print(f"{path}{f':{lineno}' if lineno else ''}  {message}")
    print(f"  crates-io: {len(found)} finding(s) over {len(crates)} crate(s)")
    return 1 if found else 0


def cargo(verb, crates, *flags):
    """Runs `cargo <verb>` over `crates`, the lock as it is; cargo's exit code."""
    packages = [arg for name, _ in crates for arg in ("--package", name)]
    return subprocess.run(["cargo", verb, "--locked", *flags, *packages], check=False).returncode


def forget_earlier_packages(crates):
    """Drops what an earlier check left of `crates` as packages, so this one builds what it packages.

    A crate whose sibling crates.io lacks is built against the sibling's package, from a registry
    cargo writes under `target/package/`. Cargo takes a registry's crate at a version never to change:
    it neither unpacks the package again over the copy in `$CARGO_HOME/registry/src/` nor rebuilds
    it, so a change to the sibling between two checks at one version would go unbuilt.
    """
    home = pathlib.Path(os.environ.get("CARGO_HOME", pathlib.Path.home() / ".cargo"))
    for name, version in crates:
        # The registry cargo writes has no name, so its copies sit in `-<hash>`, beside crates.io's.
        for unpacked in home.glob(f"registry/src/-*/{name}-{version}"):
            shutil.rmtree(unpacked)
    # A crate's builds go by its name, those from the registry's copy among them.
    packages = [arg for name, _ in crates for arg in ("--package", name)]
    subprocess.run(["cargo", "clean", "--quiet", *packages], check=True)


def check():
    """Packages every publishable crate, and builds each from its package."""
    crates = publishable()
    if not crates:
        print("crates-io: the workspace publishes no crate")
        return 0
    # The working tree as it is: a check runs before its changes are committed.
    flags = ["--allow-dirty"]
    if any(published(name, version) for name, version in crates):
        print("crates-io: crates.io has these versions already; packaging without the build")
        flags.append("--no-verify")
    else:
        forget_earlier_packages(crates)
    return cargo("package", crates, *flags)


def publish():
    """Publishes every publishable crate crates.io does not have yet."""
    crates = publishable()
    remaining = [(name, version) for name, version in crates if not published(name, version)]
    for name, version in crates:
        if (name, version) not in remaining:
            print(f"crates-io: {name} {version} is on crates.io already")
    # A publish is of commits alone, so cargo refuses a working tree with changes.
    return cargo("publish", remaining) if remaining else 0


if __name__ == "__main__":
    args = sys.argv[1:]
    if args[:1] == ["metadata"] and args[1:2] in ([], ["--book"]) and len(args) in (1, 3):
        sys.exit(metadata(args[2] if len(args) == 3 else None))
    commands = {"check": check, "publish": publish}
    if len(args) != 1 or args[0] not in commands:
        sys.exit(__doc__)
    sys.exit(commands[args[0]]())
