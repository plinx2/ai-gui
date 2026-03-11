import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import type { Playbook } from "../types";

export function usePlaybooks() {
  const [playbooks, setPlaybooks] = useState<Playbook[]>([]);
  const [loading, setLoading] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const data = await api.getPlaybooks();
      setPlaybooks(data.sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const save = useCallback(
    async (playbook: Playbook) => {
      await api.savePlaybook(playbook);
      await load();
    },
    [load],
  );

  const remove = useCallback(
    async (id: string) => {
      await api.deletePlaybook(id);
      setPlaybooks((prev) => prev.filter((p) => p.id !== id));
    },
    [],
  );

  return { playbooks, loading, save, remove, reload: load };
}
