import { useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { MoreHorizontal } from "lucide-react";
import {
  Button,
  confirm,
  EmptyState,
  icon,
  IconButton,
  ListGroup,
  ListRow,
  LoadingState,
  Menu,
  MenuContent,
  MenuItem,
  MenuTrigger,
  SearchField,
  Table,
  Text,
  toast,
} from "@lettuceai/crisp";
import type { LocalModelFile, ProviderAccountView } from "@/api/generated/bindings";
import { revealLocalModel } from "@/api/local-model-files";
import { queryKeys } from "@/api/query-keys";
import { minimalModelConfig } from "@/entities/model";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsAction, useSettingsApi } from "../data";
import { useByteSize } from "../image/local-data";
import { FolderMoves } from "./FolderMoves";
import { useFolderMoves } from "./folder-move-data";
import { browserKey } from "./data";

export function DownloadedModels({ account }: { account: ProviderAccountView | undefined }) {
  return account ? <OllamaDownloaded key={account.id} account={account} /> : <LocalDownloaded />;
}
function LocalDownloaded() {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const navigate = useNavigate();
  const client = useQueryClient();
  const action = useSettingsAction();
  const size = useByteSize();
  const [search, setSearch] = useState("");
  const createOperations = useRef(new Map<string, string>());
  const providerOperation = useRef(crypto.randomUUID());
  const files = useQuery({
    queryKey: queryKeys.localModels.all,
    queryFn: () => api.call("localModelsList"),
  });
  const dir = useQuery({
    queryKey: [...queryKeys.localModels.all, "directory"],
    queryFn: () => api.call("localModelsDirGet"),
  });
  const folderMoves = useFolderMoves();
  const rows =
    files.data?.files.filter((file) =>
      `${file.filename} ${file.repo} ${file.architecture ?? ""}`
        .toLowerCase()
        .includes(search.trim().toLowerCase()),
    ) ?? [];
  const refresh = async () => {
    await Promise.all([
      client.invalidateQueries({ queryKey: queryKeys.localModels.all }),
      client.invalidateQueries({ queryKey: browserKey }),
    ]);
  };
  const deleteFile = async (file: LocalModelFile) => {
    const names = file.used_by.map((reference) => reference.display_name ?? t("hf.globalDefaults"));
    if (
      !(await confirm({
        title: t("hf.deleteTitle"),
        message: [
          t("hf.deleteBody", { name: file.filename }),
          names.length ? t("hf.deleteUsed", { models: names.join(", ") }) : null,
        ]
          .filter(Boolean)
          .join(" "),
        confirmLabel: t("hf.delete"),
        cancelLabel: t("manage.cancel"),
        destructive: true,
      }))
    )
      return;
    const result = await api.call("localModelDelete", { path: file.path });
    if (result.referencing_profiles.length > 0)
      toast.warning(
        t("hf.deleteUsed", {
          models: result.referencing_profiles
            .map((reference) => reference.display_name ?? t("hf.globalDefaults"))
            .join(", "),
        }),
      );
    await refresh();
  };
  const openFile = async (file: LocalModelFile) => {
    const reference = file.used_by.find((item) => item.model_profile_id);
    if (reference?.model_profile_id) {
      await navigate({
        to: "/settings/models/$modelId",
        params: { modelId: reference.model_profile_id },
      });
      return;
    }
    const currentAccounts = await client.fetchQuery({
      queryKey: queryKeys.providers.accounts(),
      queryFn: () => api.call("providerAccountsList"),
      staleTime: 0,
    });
    let localAccount =
      currentAccounts.find((item) => item.provider_kind === "llamacpp" && item.enabled) ??
      currentAccounts.find((item) => item.provider_kind === "llamacpp");
    if (!localAccount?.enabled) {
      const existing = localAccount;
      localAccount = await api.call("providerAccountSave", {
        account: {
          id: existing?.id ?? null,
          provider_kind: "llamacpp",
          label: existing?.label ?? t("hf.localProvider"),
          base_url: existing?.base_url ?? null,
          enabled: true,
          streaming_enabled: existing?.streaming_enabled ?? true,
          allow_invalid_tls: existing?.allow_invalid_tls ?? false,
          config: existing?.config ?? { kind: "standard" },
        },
        api_key: null,
        clear_api_key: false,
        expected_revision: existing?.revision ?? null,
        client_operation_id: providerOperation.current,
      });
      const saved = localAccount;
      client.setQueryData<ProviderAccountView[]>(queryKeys.providers.accounts(), (current) => [
        ...(current ?? []).filter((account) => account.id !== saved.id),
        saved,
      ]);
    }
    const operation = createOperations.current.get(file.path) ?? crypto.randomUUID();
    createOperations.current.set(file.path, operation);
    const created = await api.call("modelSave", {
      model: {
        id: null,
        provider_account_id: localAccount.id,
        external_model_id: file.path,
        display_name: file.filename.replace(/\.gguf$/i, ""),
        kind: "chat",
        config: minimalModelConfig,
        input_scopes: ["text"],
        output_scopes: ["text"],
        remote_metadata: null,
      },
      expected_revision: null,
      client_operation_id: operation,
    });
    await navigate({
      to: "/settings/models/$modelId",
      params: { modelId: created.id },
    });
  };
  const move = () =>
    action.mutate(async () => {
      const chosen = await api.call("filesPickDirectory");
      if (!chosen.path || chosen.path === dir.data?.path) return;
      if (
        !(await confirm({
          title: t("hf.folder"),
          message: t("hf.folderBody"),
          confirmLabel: t("hf.move"),
          cancelLabel: t("manage.cancel"),
        }))
      )
        return;
      const accepted = await api.call("localModelsDirSet", {
        path: chosen.path,
        move_existing: true,
        client_operation_id: crypto.randomUUID(),
      });
      folderMoves.admit(accepted.job_id);
      await refresh();
    });
  const menus = (file: LocalModelFile) => (
    <Menu>
      <MenuTrigger>
        <IconButton label={file.filename}>
          <MoreHorizontal size={icon.md} />
        </IconButton>
      </MenuTrigger>
      <MenuContent align="end">
        {((!file.is_mmproj && !file.is_mtp && !file.is_dflash) ||
          file.used_by.some((reference) => reference.model_profile_id)) && (
          <MenuItem onSelect={() => action.mutate(() => openFile(file))}>
            {t(
              file.used_by.some((reference) => reference.model_profile_id)
                ? "hf.openModel"
                : "hf.makeModel",
            )}
          </MenuItem>
        )}
        <MenuItem
          onSelect={() =>
            action.mutate(async () => {
              await navigator.clipboard.writeText(file.path);
              toast.success(t("hf.copied"));
            })
          }
        >
          {t("hf.copyPath")}
        </MenuItem>
        <MenuItem onSelect={() => action.mutate(() => revealLocalModel(file.path))}>
          {t("hf.showFolder")}
        </MenuItem>
        <MenuItem danger onSelect={() => action.mutate(() => deleteFile(file))}>
          {t("hf.delete")}
        </MenuItem>
      </MenuContent>
    </Menu>
  );
  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        <SearchField
          className="min-w-0 flex-1"
          value={search}
          onChange={setSearch}
          aria-label={t("hf.search")}
          placeholder={t("hf.search")}
        />
        <Button
          variant="secondary"
          loading={action.isPending}
          onClick={() =>
            action.mutate(async () => {
              const picked = await api.call("filesPickOpen", {
                kinds: ["gguf_model"],
                multiple: true,
              });
              for (const source of picked.sources)
                await api.call("localModelAdopt", { source, model_name: null });
              await refresh();
            })
          }
        >
          {t("hf.addFile")}
        </Button>
      </div>
      {dir.isPending ? (
        <LoadingState />
      ) : dir.isError ? (
        <FailureState error={dir.error} onRetry={() => void dir.refetch()} />
      ) : (
        <ListGroup>
          <ListRow
            title={t("hf.folder")}
            description={dir.data.path}
            trailing={
              <Button
                size="sm"
                variant="ghost"
                disabled={action.isPending || folderMoves.active}
                onClick={move}
              >
                {t("hf.change")}
              </Button>
            }
          />
          <ListRow
            title={t("hf.size")}
            trailing={size(files.data?.files.reduce((sum, file) => sum + file.size, 0) ?? 0)}
          />
        </ListGroup>
      )}
      <FolderMoves state={folderMoves} />
      {files.isPending ? (
        <LoadingState />
      ) : files.isError ? (
        <FailureState error={files.error} onRetry={() => void files.refetch()} />
      ) : (
        <Table
          rows={rows}
          rowKey={(file) => file.path}
          empty={<EmptyState title={t("hf.noDownloads")} description={t("hf.noDownloadsBody")} />}
          columns={[
            {
              key: "file",
              header: t("hf.file"),
              cell: (file) => (
                <div className="min-w-0">
                  <Text className="block max-w-80 truncate">{file.filename}</Text>
                  <Text variant="hint" className="block truncate">
                    {file.repo}
                  </Text>
                </div>
              ),
            },
            {
              key: "kind",
              header: t("hf.kind"),
              cell: (file) =>
                file.is_mmproj
                  ? t("hf.visionKind")
                  : file.is_mtp
                    ? t("hf.mtpKind")
                    : file.is_dflash
                      ? t("hf.dflashKind")
                      : file.quantization,
            },
            {
              key: "arch",
              header: t("hf.arch"),
              cell: (file) => file.architecture ?? t("hf.unknown"),
            },
            {
              key: "size",
              header: t("hf.size"),
              numeric: true,
              cell: (file) => size(file.size),
            },
            {
              key: "used",
              header: t("hf.usedBy"),
              cell: (file) =>
                file.used_by
                  .map((item) => item.display_name ?? t("hf.globalDefaults"))
                  .join(", ") || t("hf.unused"),
            },
            { key: "menu", header: t("hf.file"), cell: menus },
          ]}
        />
      )}
      {action.isError && <FailureState error={action.error} size="sm" />}
    </div>
  );
}
function OllamaDownloaded({ account }: { account: ProviderAccountView }) {
  const { t, i18n } = useTranslation("settings");
  const api = useSettingsApi();
  const size = useByteSize();
  const action = useSettingsAction();
  const [search, setSearch] = useState("");
  const models = useQuery({
    queryKey: [...browserKey, "ollama", account.id],
    queryFn: () => api.call("ollamaModelsList", { provider_account_id: account.id }),
  });
  return (
    <div className="space-y-4">
      <SearchField
        value={search}
        onChange={setSearch}
        placeholder={t("hf.search")}
        aria-label={t("hf.search")}
      />
      {models.isPending ? (
        <LoadingState />
      ) : models.isError ? (
        <FailureState error={models.error} onRetry={() => void models.refetch()} />
      ) : (
        <Table
          rows={models.data.models.filter((item) =>
            item.name.toLowerCase().includes(search.toLowerCase()),
          )}
          rowKey={(item) => item.name}
          empty={<EmptyState title={t("hf.noDownloads")} />}
          columns={[
            { key: "name", header: t("hf.name"), cell: (item) => item.name },
            {
              key: "family",
              header: t("hf.family"),
              cell: (item) => item.family ?? t("hf.unknown"),
            },
            {
              key: "params",
              header: t("hf.params"),
              cell: (item) => item.parameter_size ?? t("hf.unknown"),
            },
            {
              key: "quant",
              header: t("hf.quant"),
              cell: (item) => item.quantization_level ?? t("hf.unknown"),
            },
            {
              key: "size",
              header: t("hf.size"),
              numeric: true,
              cell: (item) => (item.size === null ? t("hf.unknown") : size(item.size)),
            },
            {
              key: "changed",
              header: t("hf.changed"),
              cell: (item) =>
                item.modified_at
                  ? new Intl.DateTimeFormat(i18n.language, {
                      dateStyle: "medium",
                    }).format(new Date(item.modified_at))
                  : t("hf.unknown"),
            },
            {
              key: "delete",
              header: t("hf.delete"),
              cell: (item) => (
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={action.isPending}
                  onClick={() =>
                    action.mutate(async () => {
                      if (
                        !(await confirm({
                          title: t("hf.deleteServer", { name: item.name }),
                          confirmLabel: t("hf.delete"),
                          cancelLabel: t("manage.cancel"),
                          destructive: true,
                        }))
                      )
                        return;
                      await api.call("ollamaModelDelete", {
                        provider_account_id: account.id,
                        model: item.name,
                      });
                      await models.refetch();
                    })
                  }
                >
                  {t("hf.delete")}
                </Button>
              ),
            },
          ]}
        />
      )}
      {action.isError && <FailureState error={action.error} size="sm" />}
    </div>
  );
}
