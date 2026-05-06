import { useEffect, useState } from 'react';

export function useMiniPay() {
  const [isMiniPay, setIsMiniPay] = useState(false);
  const [address, setAddress] = useState('');

  useEffect(() => {
    const eth = (window as any).ethereum;
    if (eth?.isMiniPay) {
      setIsMiniPay(true);
      eth.request({ method: 'eth_requestAccounts' }).then((accounts: string[]) => {
        setAddress(accounts[0]);
      });
    }
  }, []);

  return { isMiniPay, address };
}
