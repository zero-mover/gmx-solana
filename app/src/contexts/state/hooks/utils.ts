import { State, StateCtx } from "../StateProvider"
import { useContextSelector, useContext, Context } from "use-context-selector";

export const useStateSelector = <T>(selector: (s: State) => T) => {
  const state = useContext(StateCtx);
  if (!state) {
    throw new Error("Use `useStateSelector` outside of `StateProvider`");
  }
  return useContextSelector(StateCtx as Context<State>, selector);
}
