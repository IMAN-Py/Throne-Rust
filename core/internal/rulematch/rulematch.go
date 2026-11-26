package rulematch

import (
	"strings"

	"github.com/sagernet/sing-box/adapter"
	C "github.com/sagernet/sing-box/constant"
	"github.com/sagernet/sing-box/option"
	R "github.com/sagernet/sing-box/route/rule"
	"github.com/sagernet/sing/common/json/badoption"
)

// Entry is one value of a route rule, matched by the very item sing-box builds for it.
type Entry struct {
	key  string
	item R.RuleItem
}

type domainExactItem struct {
	value string
	desc  string
}

func (d domainExactItem) Match(metadata *adapter.InboundContext) bool {
	host := metadata.Domain
	if host == "" {
		host = metadata.Destination.Fqdn
	}
	return host != "" && strings.EqualFold(host, d.value)
}

func (d domainExactItem) String() string { return d.desc }

type domainSuffixItem struct {
	value  string
	hasDot bool
	desc   string
}

func (d domainSuffixItem) Match(metadata *adapter.InboundContext) bool {
	host := metadata.Domain
	if host == "" {
		host = metadata.Destination.Fqdn
	}
	if host == "" {
		return false
	}
	n, m := len(host), len(d.value)
	if d.hasDot {
		return n >= m && strings.EqualFold(host[n-m:], d.value)
	}
	if n == m {
		return strings.EqualFold(host, d.value)
	}
	return n > m && host[n-m-1] == '.' && strings.EqualFold(host[n-m:], d.value)
}

func (d domainSuffixItem) String() string { return d.desc }

// ruleSetEntry probes one set of a rule_set list, the way sing-box's own item probes each of its sets.
type ruleSetEntry struct {
	tag         string
	set         adapter.RuleSet
	matchSource bool
}

func (e ruleSetEntry) Match(metadata *adapter.InboundContext) bool {
	probe := *metadata
	probe.ResetRuleCache()
	probe.IPCIDRMatchSource = e.matchSource
	return e.set.Match(&probe)
}

func (e ruleSetEntry) String() string {
	return "rule_set=" + e.tag
}

// Entries splits a default route rule into one entry per value, so the one that matched can be named:
// the rule's own String() keeps only the first three values of every list.
// Nil for a logical or inverted rule, which has no single entry to blame.
func Entries(rule option.Rule, ruleSet func(tag string) (adapter.RuleSet, bool)) []Entry {
	if rule.Type != "" && rule.Type != C.RuleTypeDefault {
		return nil
	}
	r := rule.DefaultOptions.RawDefaultRule
	if r.Invert {
		return nil
	}

	var entries []Entry
	add := func(key string, item R.RuleItem, err error) {
		if err == nil {
			entries = append(entries, Entry{key, item})
		}
	}
	for _, v := range r.Domain {
		if v == "" {
			continue
		}
		entries = append(entries, Entry{
			key:  "domain",
			item: domainExactItem{value: v, desc: "domain=" + v},
		})
	}
	for _, v := range r.DomainSuffix {
		if v == "" {
			continue
		}
		entries = append(entries, Entry{
			key:  "domain_suffix",
			item: domainSuffixItem{value: v, hasDot: v[0] == '.', desc: "domain_suffix=" + v},
		})
	}
	for _, v := range r.DomainKeyword {
		add("domain_keyword", R.NewDomainKeywordItem([]string{v}), nil)
	}
	for _, v := range r.DomainRegex {
		item, err := R.NewDomainRegexItem([]string{v})
		add("domain_regex", item, err)
	}
	for _, v := range r.IPCIDR {
		item, err := R.NewIPCIDRItem(false, []*badoption.Prefixable{v})
		add("ip_cidr", item, err)
	}
	if r.IPIsPrivate {
		add("ip_is_private", R.NewIPIsPrivateItem(false), nil)
	}
	for _, tag := range r.RuleSet {
		if set, found := ruleSet(tag); found {
			add("rule_set", ruleSetEntry{tag, set, r.RuleSetIPCIDRMatchSource}, nil)
		}
	}
	for _, v := range r.ProcessName {
		add("process_name", R.NewProcessItem([]string{v}), nil)
	}
	for _, v := range r.ProcessPath {
		add("process_path", R.NewProcessPathItem([]string{v}), nil)
	}
	for _, v := range r.ProcessPathRegex {
		item, err := R.NewProcessPathRegexItem([]string{v})
		add("process_path_regex", item, err)
	}
	// Only Android fills a connection's package names, so elsewhere these never match.
	for _, v := range r.PackageName {
		add("package_name", R.NewPackageNameItem([]string{v}), nil)
	}
	for _, v := range r.PackageNameRegex {
		item, err := R.NewPackageNameRegexItem([]string{v})
		add("package_name_regex", item, err)
	}
	return entries
}

// Matched names the first matching value of every field, e.g. "domain_suffix=github.com process_name=chrome.exe".
func Matched(entries []Entry, metadata adapter.InboundContext) string {
	if len(entries) == 0 {
		return ""
	}
	metadata.ResetRuleCache()
	var first string
	var b strings.Builder
	matchedKey := ""
	for _, entry := range entries {
		if entry.key == matchedKey || !entry.item.Match(&metadata) {
			continue
		}
		s := entry.item.String()
		if first == "" {
			first = s
		} else {
			if b.Len() == 0 {
				b.Grow(len(first) + 1 + len(s))
				b.WriteString(first)
			}
			b.WriteByte(' ')
			b.WriteString(s)
		}
		matchedKey = entry.key
	}
	if b.Len() > 0 {
		return b.String()
	}
	return first
}
