package rulematch

import (
	"net/netip"
	"testing"

	"github.com/sagernet/sing-box/adapter"
	"github.com/sagernet/sing-box/option"
	R "github.com/sagernet/sing-box/route/rule"
	"github.com/sagernet/sing/common/json/badoption"
	M "github.com/sagernet/sing/common/metadata"
)

func TestMatchedZeroOneAndMulti(t *testing.T) {
	var r option.Rule
	r.DefaultOptions.DomainSuffix = []string{"github.com", "example.org"}
	r.DefaultOptions.ProcessName = []string{"chrome.exe"}
	p := badoption.Prefixable(netip.MustParsePrefix("10.0.0.0/8"))
	r.DefaultOptions.IPCIDR = []*badoption.Prefixable{&p}

	entries := Entries(r, func(string) (adapter.RuleSet, bool) { return nil, false })

	if got := Matched(nil, adapter.InboundContext{}); got != "" {
		t.Fatalf("empty entries: got %q, want empty", got)
	}
	if got := Matched(entries, adapter.InboundContext{Domain: "nomatch.net"}); got != "" {
		t.Fatalf("no match: got %q, want empty", got)
	}
	single := Matched(entries, adapter.InboundContext{Domain: "api.github.com"})
	if single != "domain_suffix=github.com" {
		t.Fatalf("single match: got %q", single)
	}
	multi := Matched(entries, adapter.InboundContext{
		Domain:      "api.github.com",
		Destination: M.ParseSocksaddrHostPort("10.1.2.3", 443),
	})
	if multi != "domain_suffix=github.com ip_cidr=10.0.0.0/8" {
		t.Fatalf("multi match: got %q", multi)
	}
}

func TestDomainExactAndSuffixDifferentialParity(t *testing.T) {
	patterns := []string{"github.com", ".github.com", "a.b.c", ".com"}
	hosts := []string{
		"",
		"github.com",
		"GITHUB.COM",
		"api.github.com",
		"API.GitHub.Com",
		"notgithub.com",
		"x.a.b.c",
		"a.b.c",
		"ba.b.c",
		"com",
		"example.com",
	}

	for _, pat := range patterns {
		refExact, err := R.NewDomainItem([]string{pat}, nil)
		if err != nil {
			t.Fatal(err)
		}
		refSuffix, err := R.NewDomainItem(nil, []string{pat})
		if err != nil {
			t.Fatal(err)
		}

		eExact := Entries(option.Rule{DefaultOptions: option.DefaultRule{RawDefaultRule: option.RawDefaultRule{Domain: []string{pat}}}}, nil)
		eSuffix := Entries(option.Rule{DefaultOptions: option.DefaultRule{RawDefaultRule: option.RawDefaultRule{DomainSuffix: []string{pat}}}}, nil)

		for _, h := range hosts {
			for _, meta := range []adapter.InboundContext{
				{Domain: h},
				{Destination: M.Socksaddr{Fqdn: h}},
			} {
				wantExact := ""
				if refExact.Match(&meta) {
					wantExact = refExact.String()
				}
				if gotExact := Matched(eExact, meta); gotExact != wantExact {
					t.Fatalf("exact pat=%q host=%q fqdn=%q: got %q, want %q", pat, meta.Domain, meta.Destination.Fqdn, gotExact, wantExact)
				}

				wantSuffix := ""
				if refSuffix.Match(&meta) {
					wantSuffix = refSuffix.String()
				}
				if gotSuffix := Matched(eSuffix, meta); gotSuffix != wantSuffix {
					t.Fatalf("suffix pat=%q host=%q fqdn=%q: got %q, want %q", pat, meta.Domain, meta.Destination.Fqdn, gotSuffix, wantSuffix)
				}
			}
		}
	}
}

func BenchmarkMatchedSingle(b *testing.B) {
	var r option.Rule
	for i := 0; i < 40; i++ {
		r.DefaultOptions.DomainSuffix = append(r.DefaultOptions.DomainSuffix, "example"+string(rune('a'+(i%26)))+".com")
	}
	r.DefaultOptions.DomainSuffix = append(r.DefaultOptions.DomainSuffix, "github.com")
	entries := Entries(r, func(string) (adapter.RuleSet, bool) { return nil, false })
	meta := adapter.InboundContext{Domain: "gist.github.com"}
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		if Matched(entries, meta) == "" {
			b.Fatal("expected match")
		}
	}
}
