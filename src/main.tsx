import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { api } from './api/api';
import { App } from './App';
import { StoreProvider, errorMessage } from './state';
import './styles.css';

const root = createRoot(document.getElementById('root')!);

async function start() {
  try {
    const initial = await api.bootstrap();
    root.render(
      <StrictMode>
        <StoreProvider initial={initial}>
          <App />
        </StoreProvider>
      </StrictMode>,
    );
  } catch (e) {
    root.render(
      <div className="fatal">
        <h1>Payback couldn’t open its data</h1>
        <p>{errorMessage(e)}</p>
        <p className="muted">
          Your data file hasn’t been changed. Copies from the last two weeks are in the “backups” folder next to it.
        </p>
        <button className="btn btn-primary" onClick={() => void start()}>
          Try again
        </button>
      </div>,
    );
  }
}

void start();
