import { useEffect, useState } from "react";
import { getBackend, type WindowControls } from "../lib/backend";

export function useWindowControls(): WindowControls | null {
  const [controls, setControls] = useState<WindowControls | null>(null);
  useEffect(() => {
    getBackend().then((b) => setControls(b.window));
  }, []);
  return controls;
}
