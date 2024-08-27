import { useState } from 'react'
import reactLogo from './assets/react.svg'
import viteLogo from '/vite.svg'
import './App.css'


async function getVersionFn(a: number) {
  const resp = await fetch(`/api/version?a=${a || 0}`, {
    headers: {
      'Content-Type': 'application/json'
    }
  })
  return await resp.json()
}

function App() {
  const [count, setCount] = useState(0)
  const [version, setVersion] = useState(null)

  async function getVersion() {
    const result = await getVersionFn(count)
    setVersion(result.data)
  }

  return (
    <>
      <div>
        <a href="https://vitejs.dev" target="_blank">
          <img src={viteLogo} className="logo" alt="Vite logo" />
        </a>
        <a href="https://react.dev" target="_blank">
          <img src={reactLogo} className="logo react" alt="React logo" />
        </a>
      </div>
      <h1>Vite + React</h1>
      <div className="card">
        <button onClick={() => setCount((count) => count + 1)}>
          count is {count}
        </button>
        <p>
          Edit <code>src/App.tsx</code> and save to test HMR
        </p>
      </div>
      <p className="read-the-docs">
        Click on the Vite and React logos to learn more
      </p>
      <div>version: {version}</div>
      <button onClick={getVersion}>get version</button>
    </>
  )
}

export default App
