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
          pattern="[a-zA-Z0-9.:-]+"
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

        {#if accountInfo.failure_count > 0}
          <p class="red">We have encountered {accountInfo.failure_count} fatal errors when trying to sync. After 10 attempted sync attempts, we will stop synchronizing.</p>
        {/if}

        {#if accountInfo.last_error}
          <p class="red">The last error we encountered was: <code>{accountInfo.last_error}</code></p>
        {/if}

        <p>Your lists will be updated once per day automatically. Take a look at the <a href="https://github.com/untitaker/mastodon-list-bot">README</a> to see which list names are supported.</p>

        {#if accountInfo.last_success_at}
          <p>Your last successful sync was at <code>{accountInfo.last_success_at}</code></p>
        {/if}

        <p>You can also <button on:click={() => uiSyncImmediate(accountInfo)}>trigger a list sync manually</button>

        {#if syncImmediatePromise != null}
          {#await syncImmediatePromise}
            <p><code>loading...</code></p>
          {:then syncImmediateResult}
            <p><code>{JSON.stringify(syncImmediateResult)}</p>
          {/await}
        {/if}
    </div>
  {/if}
{/await}
