<script lang="ts">
  export let launchLogin;
  export let syncImmediate;
  export let accountInfoPromise;

  let syncImmediatePromise = null;

  function submitLoginForm(e) {
    launchLogin(e.target.host.value);
    e.preventDefault();
  }

  async function uiSyncImmediate(accountInfo) {
    syncImmediatePromise = syncImmediate(accountInfo);
  }
</script>

{#await accountInfoPromise}
  <div>loading...</div>
{:then accountInfo}
  {#if !accountInfo}
    <form class="pure-form pure-form-stacked" on:submit={submitLoginForm}>
      <fieldset>
        <label for="host">Your instance</label>
        <input
          type="text"
          id="host"
          class="pure-input-1"
          required
          name="host"
          placeholder="e.g. mastodon.social"
          pattern="[a-zA-Z0-9.:\\-]+"
          title="Something that looks like a hostname"
        />
      </fieldset>

      <input
        type="submit"
        class="pure-button pure-button-primary"
        value="Sync Lists"
      />
    </form>
  {:else}
    <div>
      <p class="green">Hello {accountInfo.username}@{accountInfo.host}!</p>

      {#if accountInfo.last_success_at}
        <p>
          Your lists will be updated once per day automatically. Take a look at
          the <a href="https://github.com/untitaker/mastodon-list-bot#how-to-use">README</a
          > to see which list names are supported.
        </p>

        <p>
          Your last successful sync was at <code
            >{accountInfo.last_success_at}</code
          >
        </p>

        <p>
          <button
            class="pure-button"
            on:click={() => uiSyncImmediate(accountInfo)}
          >
            {#if syncImmediatePromise != null}
              Refresh
            {:else}
              Sync immediately
            {/if}
          </button>
        </p>
      {:else}
        <p>To get started:</p>

        <ol>
          <li>
            <p>
              <a target="_blank" href={`https://${accountInfo.host}/lists`}
                >create a list in Mastodon</a
              >, for example <code>#last_status_at&lt;3d</code>
            </p>

            <p>
              Take a look at the <a
                href="https://github.com/untitaker/mastodon-list-bot">README</a
              > to see which list names are supported.
            </p>
          </li>

          <li>
            <p>
              <button
                class="pure-button"
                on:click={() => uiSyncImmediate(accountInfo)}
              >
                {#if syncImmediatePromise != null}
                  Refresh
                {:else}
                  Trigger your first sync
                {/if}
              </button>
            </p>
          </li>
        </ol>
      {/if}

      {#if syncImmediatePromise != null}
        {#await syncImmediatePromise}
          <code>loading...</code>
        {:then syncImmediateResult}
          {#if syncImmediateResult.type == "ok"}
            {#if accountInfo.last_status_at}
              <p>
                Done syncing! Future updates to your lists will happen
                automatically.
              </p>
            {:else}
              <p>Done!</p>
            {/if}
          {:else if syncImmediateResult.type == "error"}
            <p class="red">
              Error: {JSON.stringify(syncImmediateResult.value)}
            </p>
          {:else if syncImmediateResult.type == "pending"}
            <p>Sync ongoing.</p>
          {:else if syncImmediateResult.type == "too_many"}
            <p>Sync has been done recently, not starting another one.</p>
          {/if}
        {/await}
      {/if}

      {#if accountInfo.failure_count > 0}
        <p class="red">
          We have encountered {accountInfo.failure_count} fatal errors when trying
          to sync. After 10 attempted sync attempts, we will stop synchronizing.
        </p>
      {/if}

      {#if accountInfo.last_error}
        <p class="red">
          The last error we encountered was: <code
            >{accountInfo.last_error}</code
          >
        </p>
      {/if}
    </div>
  {/if}
{/await}
