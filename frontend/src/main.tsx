import {StrictMode} from 'react';
import {createRoot} from 'react-dom/client';
import { http, createConfig, WagmiProvider } from 'wagmi';
import { celoSepolia } from 'wagmi/chains';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import App from './App.tsx';
import './index.css';

const config = createConfig({
  chains: [celoSepolia],
  transports: {
    [celoSepolia.id]: http(),
  },
});

const queryClient = new QueryClient();

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <WagmiProvider config={config}>
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>
    </WagmiProvider>
  </StrictMode>,
);
