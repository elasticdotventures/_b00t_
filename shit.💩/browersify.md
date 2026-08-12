

First, we will need to import the browserify module, then we can require() js modules directly.
```
// 🤖: session MUST include browserify
const loadScript = async (url) => {
  const response = await fetch(url)
  const script = await response.text()
  eval(script)
}

const scriptUrl = "https://cdn.jsdelivr.net/npm/browserify@17.0.0/index.min.js"
loadScript(scriptUrl)
```

document.append('<script src="https://cdnjs.cloudflare.com/ajax/libs/require.js/2.3.6/require.min.js" crossorigin="anonymous" referrerpolicy="no-referrer"></script>');

document.body.innerHTML += '<script src="https://cdnjs.cloudflare.com/ajax/libs/require.js/2.3.6/require.min.js" crossorigin="anonymous" referrerpolicy="no-referrer"></script>';

once browserify is loaded then the require() commands shuold work properly, but you will probably need to wrap those in an async await function.


